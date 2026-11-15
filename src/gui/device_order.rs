//! BUSID 数字倒序；所有扫描入口共用，缺失/无效 BUSID 排在最后。
use std::cmp::Reverse;
use usbipd_tool::core::models::UsbDevice;

fn numeric_busid(busid: &str) -> Option<Vec<u64>> {
    let (bus, path) = busid.trim().split_once('-')?;
    let mut components = Vec::new();
    for part in std::iter::once(bus).chain(path.split('.')) {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        components.push(part.parse().ok()?);
    }
    Some(components)
}

pub fn sort_by_busid_descending(devices: &mut [UsbDevice]) {
    devices.sort_by_cached_key(|device| {
        let numeric = numeric_busid(&device.busid);
        // 数字倒序；相同数字使用原 BUSID 字符串打破平局，不改变设备内容。
        (numeric.is_none(), Reverse(numeric.unwrap_or_default()), device.busid.clone())
    });
}
