//! 高层操作：封装 usbipd 子命令（与原版 README 中的命令映射一致）。

use super::models::{UsbDevice, WslDistro};
use super::parser::{parse_list_text, parse_state_json, parse_wsl_default_distro, parse_wsl_distros_quiet};
use super::usbipd::{self, CommandResult};

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// 扫描设备。优先 `usbipd state`（JSON），回退 `usbipd list`。
/// 返回 (设备列表, 警告文本)；失败时 Err(错误文本)。
pub fn scan_devices() -> Result<(Vec<UsbDevice>, Option<String>), String> {
    let res = usbipd::run_usbipd(&s(&["state"]), 30);
    if res.ok() {
        if let Ok(devs) = parse_state_json(&res.stdout) {
            let warn = Some(res.stderr.trim().to_string()).filter(|w| !w.is_empty());
            return Ok((devs, warn));
        }
    }
    let res2 = usbipd::run_usbipd(&s(&["list"]), 30);
    if res2.ok() {
        let warn = Some(res2.stderr.trim().to_string()).filter(|w| !w.is_empty());
        return Ok((parse_list_text(&res2.stdout), warn));
    }
    let e = res2.error_text();
    Err(if e.is_empty() { "扫描失败".into() } else { e })
}

/// 列出 WSL 发行版并标记默认发行版；失败返回空列表。
pub fn list_wsl_distros() -> Vec<WslDistro> {
    let verbose = usbipd::run_wsl(&s(&["--list", "--verbose"]), 30);
    let default_name = if verbose.ok() { parse_wsl_default_distro(&verbose.stdout) } else { None };
    let quiet = usbipd::run_wsl(&s(&["--list", "--quiet"]), 30);
    let names = if quiet.ok() { parse_wsl_distros_quiet(&quiet.stdout) } else { Vec::new() };
    names
        .into_iter()
        .map(|n| WslDistro { is_default: Some(&n) == default_name.as_ref(), name: n })
        .collect()
}

pub fn bind_args(busid: &str, force: bool) -> Vec<String> {
    let mut a = s(&["bind", "--busid", busid]);
    if force {
        a.push("--force".into());
    }
    a
}

pub fn unbind_args(busid: &str) -> Vec<String> {
    s(&["unbind", "--busid", busid])
}

pub fn detach_args(busid: &str) -> Vec<String> {
    s(&["detach", "--busid", busid])
}

/// `usbipd attach --wsl [distro] --busid <id> [--host-ip ip] [--auto-attach]`
pub fn attach_args(busid: &str, distribution: Option<&str>, host_ip: Option<&str>, auto_attach: bool) -> Vec<String> {
    let mut a = s(&["attach", "--wsl"]);
    if let Some(d) = distribution.map(str::trim).filter(|d| !d.is_empty()) {
        a.push(d.to_string());
    }
    a.push("--busid".into());
    a.push(busid.to_string());
    if let Some(ip) = host_ip.map(str::trim).filter(|d| !d.is_empty()) {
        a.push("--host-ip".into());
        a.push(ip.to_string());
    }
    if auto_attach {
        a.push("--auto-attach".into());
    }
    a
}

pub fn bind(busid: &str, force: bool) -> CommandResult {
    usbipd::run_usbipd(&bind_args(busid, force), 60)
}

pub fn unbind(busid: &str) -> CommandResult {
    usbipd::run_usbipd(&unbind_args(busid), 60)
}

/// 普通附加（不带 --auto-attach，命令会很快返回）。
pub fn attach_wsl(busid: &str, distribution: Option<&str>, host_ip: Option<&str>) -> CommandResult {
    usbipd::run_usbipd(&attach_args(busid, distribution, host_ip, false), 60)
}

pub fn detach(busid: &str) -> CommandResult {
    usbipd::run_usbipd(&detach_args(busid), 60)
}

/// 绑定失败时是否建议使用 --force（与原版关键字一致）。
pub fn bind_needs_force_hint(error_text: &str) -> bool {
    let e = error_text.to_lowercase();
    ["incompatible", "--force", "hrdevmon", "usbpcap"].iter().any(|k| e.contains(k))
}
