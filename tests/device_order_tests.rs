//! Headless ordering regression tests: no USB commands or native windows.
#[path = "../src/gui/device_order.rs"]
mod device_order;
use usbipd_tool::core::models::{DeviceState, UsbDevice};

fn device(id: &str) -> UsbDevice {
    UsbDevice {
        busid: id.into(), vid: "1234".into(), pid: "5678".into(),
        description: format!("device-{id}"), state: DeviceState::Shared,
        client_ip: None, instance_id: Some(format!("instance-{id}")), persisted_guid: None,
    }
}
fn sorted(ids: &[&str]) -> Vec<String> {
    let mut devices: Vec<_> = ids.iter().map(|id| device(id)).collect();
    device_order::sort_by_busid_descending(&mut devices);
    devices.into_iter().map(|d| d.busid).collect()
}
#[test]
fn screenshot_busids_are_numeric_descending_not_reversed_or_lexicographic() {
    assert_eq!(sorted(&["10-1", "6-1", "7-1", "1-5"]), ["10-1", "7-1", "6-1", "1-5"]);
}
#[test]
fn multi_digit_ports_and_hub_paths_are_numeric() {
    assert_eq!(sorted(&["2-9", "2-10", "6-1.2", "6-1.10", "10-2", "6-1"]),
        ["10-2", "6-1.10", "6-1.2", "6-1", "2-10", "2-9"]);
}
#[test]
fn missing_invalid_and_overflow_busids_are_deterministic_and_last() {
    let ids = ["bad", "", "7-1", "6-", "-2", "1--2", "1-2..3", "18446744073709551616-1", "1-+2", " 10-1 "];
    let first = sorted(&ids);
    let mut shuffled = ids;
    shuffled.reverse();
    assert_eq!(first, sorted(&shuffled));
    assert_eq!(&first[..2], [" 10-1 ", "7-1"]);
    assert_eq!(first.len(), ids.len());
    assert!(sorted(&[]).is_empty());
    assert_eq!(sorted(&["1-1"]), ["1-1"]);
}
#[test]
fn rescans_do_not_toggle_order_and_device_action_identity_is_preserved() {
    let original: Vec<_> = ["10-1", "6-1", "7-1", "1-5"].iter().map(|id| device(id)).collect();
    let mut devices = original.clone();
    for _ in 0..3 {
        device_order::sort_by_busid_descending(&mut devices);
        assert_eq!(devices.iter().map(|d| d.busid.as_str()).collect::<Vec<_>>(), ["10-1", "7-1", "6-1", "1-5"]);
        for d in &devices {
            assert_eq!(d, original.iter().find(|o| o.busid == d.busid).unwrap());
        }
        let mut fresh_scan = original.clone();
        device_order::sort_by_busid_descending(&mut fresh_scan);
        assert_eq!(fresh_scan, devices);
    }
}
