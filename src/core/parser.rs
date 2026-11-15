//! 解析 usbipd / wsl 输出。

use super::models::{DeviceState, UsbDevice};
use serde::Deserialize;

fn extract_hex4_after(s: &str, tag: &str) -> String {
    // 不区分大小写查找 tag（如 "VID_"），取其后 4 位十六进制。
    let upper = s.to_ascii_uppercase();
    let tag = tag.to_ascii_uppercase();
    let mut start = 0;
    while let Some(pos) = upper[start..].find(&tag) {
        let begin = start + pos + tag.len();
        let cand: String = s[begin..].chars().take(4).collect();
        if cand.len() == 4 && cand.chars().all(|c| c.is_ascii_hexdigit()) {
            return cand.to_ascii_lowercase();
        }
        start = begin;
    }
    String::new()
}

/// 从 InstanceId（如 `USB\VID_1A86&PID_55D3\...`）提取 VID/PID（小写）。
pub fn extract_vidpid(instance_id: &str) -> (String, String) {
    (
        extract_hex4_after(instance_id, "VID_"),
        extract_hex4_after(instance_id, "PID_"),
    )
}

#[derive(Deserialize)]
struct StateRoot {
    #[serde(rename = "Devices", default)]
    devices: Option<Vec<StateDevice>>,
}

#[derive(Deserialize)]
struct StateDevice {
    #[serde(rename = "BusId", default)]
    bus_id: Option<String>,
    #[serde(rename = "ClientIPAddress", default)]
    client_ip: Option<String>,
    #[serde(rename = "Description", default)]
    description: Option<String>,
    #[serde(rename = "InstanceId", default)]
    instance_id: Option<String>,
    #[serde(rename = "PersistedGuid", default)]
    persisted_guid: Option<String>,
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// 解析 `usbipd state` 的 JSON 输出（usbipd 5.x 主数据源）。
pub fn parse_state_json(text: &str) -> Result<Vec<UsbDevice>, serde_json::Error> {
    let text = text.trim_start_matches('\u{feff}');
    let root: StateRoot = serde_json::from_str(text)?;
    let mut out = Vec::new();
    for item in root.devices.unwrap_or_default() {
        let busid = item.bus_id.unwrap_or_default().trim().to_string();
        if busid.is_empty() {
            continue;
        }
        let instance_id = item.instance_id.unwrap_or_default();
        let (vid, pid) = extract_vidpid(&instance_id);
        let client_ip = non_empty(item.client_ip);
        let persisted_guid = non_empty(item.persisted_guid);
        let state = if client_ip.is_some() {
            DeviceState::Attached
        } else if persisted_guid.is_some() {
            DeviceState::Shared
        } else {
            DeviceState::NotShared
        };
        out.push(UsbDevice {
            busid,
            vid,
            pid,
            description: item.description.unwrap_or_default().trim().to_string(),
            state,
            client_ip,
            instance_id: if instance_id.is_empty() { None } else { Some(instance_id) },
            persisted_guid,
        });
    }
    Ok(out)
}

fn is_vidpid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 9
        && b[4] == b':'
        && b[..4].iter().all(|c| c.is_ascii_hexdigit())
        && b[5..].iter().all(|c| c.is_ascii_hexdigit())
}

/// 解析一行设备记录：`BUSID  VID:PID  描述(可含空格)  状态(行尾)`。
fn parse_list_row(line: &str) -> Option<UsbDevice> {
    let line = line.trim_end();
    let (state, rest) = ["Not shared", "Shared", "Attached"]
        .iter()
        .find_map(|st| line.strip_suffix(st).map(|r| (*st, r)))?;
    // 状态前必须有空白
    if !rest.ends_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim_end();
    let mut it = rest.splitn(2, char::is_whitespace);
    let busid = it.next()?.trim();
    let after_busid = it.next()?.trim_start();
    if busid.is_empty() {
        return None;
    }
    let mut it2 = after_busid.splitn(2, char::is_whitespace);
    let vidpid = it2.next()?;
    if !is_vidpid(vidpid) {
        return None;
    }
    let desc = it2.next().unwrap_or("").trim();
    if desc.is_empty() {
        return None; // 与原版正则一致：描述与状态间至少有分隔
    }
    let (vid, pid) = vidpid.split_once(':')?;
    Some(UsbDevice {
        busid: busid.to_string(),
        vid: vid.to_ascii_lowercase(),
        pid: pid.to_ascii_lowercase(),
        description: desc.to_string(),
        state: DeviceState::from_list_text(state)?,
        client_ip: None,
        instance_id: None,
        persisted_guid: None,
    })
}

/// 解析 `usbipd list` 文本输出（旧版本回退）。
pub fn parse_list_text(text: &str) -> Vec<UsbDevice> {
    let mut out = Vec::new();
    let mut in_connected = false;
    for line in text.lines() {
        let stripped = line.trim().trim_start_matches('\u{feff}');
        if stripped == "Connected:" {
            in_connected = true;
            continue;
        }
        if stripped == "Persisted:" {
            break;
        }
        if !in_connected || stripped.is_empty() || stripped.starts_with("BUSID") {
            continue;
        }
        if let Some(d) = parse_list_row(line) {
            out.push(d);
        }
    }
    out
}

/// 解析 `wsl.exe --list --quiet` 输出（需先解码，UTF-16LE/UTF-8 均可）。
pub fn parse_wsl_distros_quiet(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim().trim_matches('\u{feff}').trim_matches('\0').trim())
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

fn trim_wsl_line(line: &str) -> &str {
    line.trim().trim_matches('\u{feff}').trim_matches('\0').trim()
}

/// 从 `wsl.exe --list --verbose` 输出中找出默认发行版（行首带 `*`）。
pub fn parse_wsl_default_distro(text: &str) -> Option<String> {
    for line in text.lines() {
        let s = trim_wsl_line(line);
        if s.starts_with('*') {
            let parts: Vec<&str> = s.split_whitespace().collect();
            if parts.len() >= 2 {
                return Some(parts[1].trim().to_string());
            }
        }
    }
    None
}

/// 从 `wsl.exe --list --verbose` 取出发行版名称（跳过表头；`*` 只是默认标记）。
pub fn parse_wsl_distros_verbose(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let s = trim_wsl_line(line);
        if s.is_empty() {
            continue;
        }
        let body = s.trim_start_matches('*').trim();
        if body.is_empty() {
            continue;
        }
        if body.to_ascii_uppercase().starts_with("NAME") {
            continue;
        }
        if let Some(name) = body.split_whitespace().next() {
            out.push(name.to_string());
        }
    }
    out
}

/// 从 `usbipd --version` 输出取版本号（去掉 `+Branch...` 构建后缀）。
pub fn parse_version(text: &str) -> Option<String> {
    let first = text.trim().lines().next()?.trim();
    if first.is_empty() {
        return None;
    }
    let v = first.split('+').next().unwrap_or(first);
    Some(v.chars().take(40).collect())
}
