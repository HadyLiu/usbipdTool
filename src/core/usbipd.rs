//! usbipd / wsl 命令封装：查找可执行文件、无窗口执行、超时、输出解码、管理员检测。

use super::decode::decode_output;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const USBIPD_NAME: &str = "usbipd";
pub const WSL_NAME: &str = "wsl.exe";

const KNOWN_USBIPD_PATHS: &[&str] = &[
    r"C:\Program Files\usbipd-win\usbipd.exe",
    r"C:\Program Files (x86)\usbipd-win\usbipd.exe",
];

/// Windows: 子进程不创建控制台窗口。
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub returncode: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CommandResult {
    pub fn new(returncode: i32, stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self { returncode, stdout: stdout.into(), stderr: stderr.into() }
    }
    pub fn ok(&self) -> bool {
        self.returncode == 0
    }
    pub fn error_text(&self) -> String {
        let s = if self.stderr.trim().is_empty() { &self.stdout } else { &self.stderr };
        s.trim().to_string()
    }
}

/// 管理员权限检测。
#[cfg(windows)]
pub fn is_admin() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

#[cfg(not(windows))]
pub fn is_admin() -> bool {
    false
}

/// 在 PATH 中查找可执行文件。
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let mut candidates = vec![name.to_string()];
    if cfg!(windows) && Path::new(name).extension().is_none() {
        let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into());
        for e in exts.split(';').filter(|e| !e.is_empty()) {
            candidates.push(format!("{name}{}", e.to_ascii_lowercase()));
        }
    }
    for dir in std::env::split_paths(&path) {
        for c in &candidates {
            let p = dir.join(c);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn find_usbipd() -> Option<PathBuf> {
    if let Some(p) = which(USBIPD_NAME) {
        return Some(p);
    }
    KNOWN_USBIPD_PATHS.iter().map(PathBuf::from).find(|p| p.is_file())
}

fn resolve_wsl() -> PathBuf {
    which(WSL_NAME).unwrap_or_else(|| PathBuf::from(WSL_NAME))
}

/// 构造一个不弹出控制台窗口的 Command。
pub fn hidden_command(exe: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn spawn_reader<R: Read + Send + 'static>(r: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut r) = r {
            let _ = r.read_to_end(&mut buf);
        }
        buf
    })
}

fn wait_with_timeout(mut child: Child, timeout: Duration) -> CommandResult {
    let out_h = spawn_reader(child.stdout.take());
    let err_h = spawn_reader(child.stderr.take());
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(30));
            }
            Err(_) => break None,
        }
    };
    let out = out_h.join().unwrap_or_default();
    let err = err_h.join().unwrap_or_default();
    match status {
        Some(st) => CommandResult::new(st.code().unwrap_or(-1), decode_output(&out), decode_output(&err)),
        None => CommandResult::new(-2, decode_output(&out), "命令执行超时。"),
    }
}

/// 运行任意命令，捕获输出（带超时）。
pub fn run_capture(exe: &Path, args: &[String], timeout: Duration, not_found_msg: &str) -> CommandResult {
    let mut cmd = hidden_command(exe);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    match cmd.spawn() {
        Ok(child) => wait_with_timeout(child, timeout),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => CommandResult::new(-1, "", not_found_msg),
        Err(e) => CommandResult::new(-1, "", e.to_string()),
    }
}

pub fn usbipd_exe() -> PathBuf {
    find_usbipd().unwrap_or_else(|| PathBuf::from(USBIPD_NAME))
}

pub fn run_usbipd(args: &[String], timeout_secs: u64) -> CommandResult {
    run_capture(
        &usbipd_exe(),
        args,
        Duration::from_secs(timeout_secs),
        &format!("找不到 {USBIPD_NAME} 可执行文件。"),
    )
}

pub fn run_wsl(args: &[String], timeout_secs: u64) -> CommandResult {
    run_capture(&resolve_wsl(), args, Duration::from_secs(timeout_secs), "找不到 wsl.exe。")
}

pub fn get_version() -> Option<String> {
    let res = run_usbipd(&["--version".to_string()], 15);
    if res.ok() { super::parser::parse_version(&res.stdout) } else { None }
}

/// 将参数列表格式化为日志中显示的命令行（含空格的参数加引号）。
pub fn display_command(args: &[String]) -> String {
    let mut parts = vec![USBIPD_NAME.to_string()];
    for a in args {
        if a.is_empty() || a.contains(char::is_whitespace) {
            parts.push(format!("\"{a}\""));
        } else {
            parts.push(a.clone());
        }
    }
    parts.join(" ")
}
