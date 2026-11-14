//! 后台任务：所有 usbipd / wsl 命令都在工作线程中执行，结果通过通道回到 UI 线程。

use eframe::egui;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use usbipd_tool::core::decode::decode_output;
use usbipd_tool::core::models::{UsbDevice, WslDistro};
use usbipd_tool::core::usbipd::{self, CommandResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Bind { force: bool },
    Unbind,
    Attach,
    Detach,
}

pub enum Msg {
    Version(Option<String>),
    ScanDone(Result<(Vec<UsbDevice>, Option<String>), String>),
    Distros(Vec<WslDistro>),
    OpDone { kind: OpKind, busid: String, res: CommandResult },
    /// 后台进程（--auto-attach）实时输出的一行
    Log(String),
    /// 请求重新扫描（例如 --auto-attach 启动几秒后）
    Rescan,
    /// 去抖后的 USB 插拔事件，走与「重新扫描」相同的扫描路径
    HardwareChanged,
    /// --auto-attach 进程已启动
    AutoAttachStarted { busid: String },
    /// --auto-attach 进程退出
    AutoAttachExited { busid: String, code: Option<i32>, killed: bool },
}

#[derive(Clone)]
pub struct Bus {
    pub tx: Sender<Msg>,
    pub ctx: egui::Context,
}

impl Bus {
    pub fn send(&self, m: Msg) {
        let _ = self.tx.send(m);
        self.ctx.request_repaint();
    }

    /// 在新线程中执行 `f`，把结果作为消息发回。
    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce() -> Msg + Send + 'static,
    {
        let bus = self.clone();
        std::thread::spawn(move || bus.send(f()));
    }
}

struct AutoProc {
    child: Mutex<Child>,
    killed: Mutex<bool>,
}

/// 管理 `usbipd attach ... --auto-attach` 常驻进程。
///
/// 原版用 60 秒超时的同步调用执行 attach，带 --auto-attach 时进程不会退出，
/// 会在超时后被杀掉。这里改为后台常驻、实时输出日志；分离设备或退出程序时终止。
#[derive(Default)]
pub struct AutoAttach {
    procs: Arc<Mutex<HashMap<String, Arc<AutoProc>>>>,
}

fn pump_lines<R: Read + Send + 'static>(r: R, prefix: &'static str, bus: Bus) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(r);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = decode_output(&buf);
                    let line = line.trim_end();
                    if !line.trim().is_empty() {
                        bus.send(Msg::Log(format!("{prefix}{line}")));
                    }
                }
            }
        }
    });
}

impl AutoAttach {
    pub fn is_running(&self, busid: &str) -> bool {
        self.procs.lock().map(|m| m.contains_key(busid)).unwrap_or(false)
    }

    /// 启动常驻附加进程；失败时返回 CommandResult 错误。
    pub fn start(&self, busid: &str, args: Vec<String>, bus: Bus) -> Result<(), CommandResult> {
        self.stop(busid);
        let mut cmd = usbipd::hidden_command(&usbipd::usbipd_exe());
        cmd.args(&args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| CommandResult::new(-1, "", e.to_string()))?;
        if let Some(o) = child.stdout.take() {
            pump_lines(o, "", bus.clone());
        }
        if let Some(e) = child.stderr.take() {
            pump_lines(e, "[stderr] ", bus.clone());
        }
        let proc_ = Arc::new(AutoProc { child: Mutex::new(child), killed: Mutex::new(false) });
        self.procs.lock().unwrap().insert(busid.to_string(), proc_.clone());
        bus.send(Msg::AutoAttachStarted { busid: busid.to_string() });

        let procs = self.procs.clone();
        let busid = busid.to_string();
        std::thread::spawn(move || {
            let code = loop {
                let st = proc_.child.lock().ok().and_then(|mut c| c.try_wait().ok()).flatten();
                if let Some(st) = st {
                    break st.code();
                }
                std::thread::sleep(Duration::from_millis(200));
            };
            let killed = *proc_.killed.lock().unwrap();
            if let Ok(mut m) = procs.lock() {
                if m.get(&busid).is_some_and(|p| Arc::ptr_eq(p, &proc_)) {
                    m.remove(&busid);
                }
            }
            bus.send(Msg::AutoAttachExited { busid, code, killed });
        });
        Ok(())
    }

    /// 终止某设备的常驻附加进程（若有）。返回是否终止了进程。
    pub fn stop(&self, busid: &str) -> bool {
        let p = self.procs.lock().ok().and_then(|mut m| m.remove(busid));
        if let Some(p) = p {
            *p.killed.lock().unwrap() = true;
            if let Ok(mut c) = p.child.lock() {
                let _ = c.kill();
            }
            true
        } else {
            false
        }
    }

    pub fn stop_all(&self) {
        let keys: Vec<String> = self.procs.lock().map(|m| m.keys().cloned().collect()).unwrap_or_default();
        for k in keys {
            self.stop(&k);
        }
    }
}
