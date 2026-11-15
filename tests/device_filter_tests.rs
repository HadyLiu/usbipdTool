//! Display-filter matching, persistence, cancel/save, and identity — no USB commands.
use serde_json::json;
use usbipd_tool::core::config::Config;
use usbipd_tool::core::device_filter::{
    self, FilterField, FilterRule, RuleError, CONFIG_KEY,
};
use usbipd_tool::core::models::{DeviceState, UsbDevice};

fn dev(busid: &str, vid: &str, pid: &str, desc: &str, instance: Option<&str>) -> UsbDevice {
    UsbDevice {
        busid: busid.into(),
        vid: vid.into(),
        pid: pid.into(),
        description: desc.into(),
        state: DeviceState::NotShared,
        client_ip: None,
        instance_id: instance.map(|s| s.into()),
        persisted_guid: Some(format!("guid-{busid}")),
    }
}

fn rule(field: FilterField, value: &str, enabled: bool) -> FilterRule {
    FilterRule { enabled, field, value: value.into() }
}

#[test]
fn empty_or_disabled_or_blank_rules_hide_nothing() {
    let devices = vec![
        dev("11-2", "12D1", "107E", "Huawei USB", None),
        dev("10-1", "1A86", "7523", "USB-SERIAL CH340", None),
    ];
    assert_eq!(device_filter::hidden_count(&devices, &[]), 0);
    assert_eq!(
        device_filter::visible_devices(&devices, &[], false).len(),
        2
    );
    let disabled = [rule(FilterField::DescriptionContains, "USB", false)];
    assert_eq!(device_filter::hidden_count(&devices, &disabled), 0);
    let blank = [rule(FilterField::DescriptionContains, "   ", true)];
    assert_eq!(device_filter::hidden_count(&devices, &blank), 0);
    let blank_bus = [rule(FilterField::BusId, "", true)];
    assert_eq!(device_filter::hidden_count(&devices, &blank_bus), 0);
    let bad_vid = [rule(FilterField::VidPid, "not-a-id", true)];
    assert_eq!(device_filter::hidden_count(&devices, &bad_vid), 0);
}

#[test]
fn description_contains_is_case_insensitive_and_uses_instance_when_desc_empty() {
    let devices = vec![
        dev("11-2", "12D1", "107E", "Huawei USB Modem", None),
        dev("10-1", "1A86", "7523", "USB-SERIAL CH340", None),
        dev("9-1", "0000", "0000", "", Some("USB\\VID_1234\\INSTANCE")),
    ];
    let rules = [rule(FilterField::DescriptionContains, "huawei", true)];
    let hidden: Vec<_> = devices
        .iter()
        .filter(|d| device_filter::is_hidden(d, &rules))
        .map(|d| d.busid.as_str())
        .collect();
    assert_eq!(hidden, ["11-2"]);

    let rules = [rule(FilterField::DescriptionContains, "instance", true)];
    assert!(device_filter::is_hidden(&devices[2], &rules));
    assert!(!device_filter::is_hidden(&devices[0], &rules));
}

#[test]
fn vidpid_is_exact_after_hex_normalization() {
    let d = dev("11-2", "12D1", "107E", "modem", None);
    assert!(device_filter::rule_matches(
        &rule(FilterField::VidPid, "12d1:107e", true),
        &d
    ));
    assert!(device_filter::rule_matches(
        &rule(FilterField::VidPid, "12D1:107E", true),
        &d
    ));
    // 1–4 hex digits, padded for comparison.
    assert!(device_filter::rule_matches(
        &rule(FilterField::VidPid, "12d1:7e", true),
        &dev("1-1", "12D1", "007E", "x", None)
    ));
    assert!(!device_filter::rule_matches(
        &rule(FilterField::VidPid, "12D1:107F", true),
        &d
    ));
    assert!(!device_filter::rule_matches(
        &rule(FilterField::VidPid, "12D1", true),
        &d
    ));
    assert!(device_filter::parse_vidpid("zzzz:0001").is_none());
    assert!(device_filter::parse_vidpid("12D1:107EE").is_none());
}

#[test]
fn busid_exact_is_trimmed_and_case_insensitive() {
    let d = dev("11-2.3.4", "1A86", "7523", "serial", None);
    assert!(device_filter::rule_matches(
        &rule(FilterField::BusId, "11-2.3.4", true),
        &d
    ));
    assert!(device_filter::rule_matches(
        &rule(FilterField::BusId, " 11-2.3.4 ", true),
        &d
    ));
    assert!(!device_filter::rule_matches(
        &rule(FilterField::BusId, "11-2", true),
        &d
    ));
}

#[test]
fn enabled_rules_combine_with_or() {
    let devices = vec![
        dev("11-2", "12D1", "107E", "Huawei", None),
        dev("10-1", "1A86", "7523", "CH340", None),
        dev("9-1", "046D", "C52B", "Unifying", None),
    ];
    let rules = [
        rule(FilterField::DescriptionContains, "huawei", true),
        rule(FilterField::VidPid, "1A86:7523", true),
        rule(FilterField::BusId, "8-1", false),
    ];
    let visible: Vec<_> = device_filter::visible_devices(&devices, &rules, false)
        .iter()
        .map(|d| d.busid.as_str())
        .collect();
    assert_eq!(visible, ["9-1"]);
    assert_eq!(device_filter::hidden_count(&devices, &rules), 2);

    let shown: Vec<_> = device_filter::visible_devices(&devices, &rules, true)
        .iter()
        .map(|d| d.busid.as_str())
        .collect();
    assert_eq!(shown, ["11-2", "10-1", "9-1"]);
}

#[test]
fn filtering_is_display_only_and_actions_use_device_identity_not_filtered_index() {
    // Discovery order is numeric descending, already sorted by the caller.
    let devices = vec![
        dev("11-2", "12D1", "107E", "hide-me", None),
        dev("10-1", "1A86", "7523", "keep", None),
        dev("9-1", "046D", "C52B", "hide-me-too", None),
    ];
    let rules = [rule(FilterField::DescriptionContains, "hide", true)];
    // Raw discovery list is unchanged — hotplug / auto-restore must still see hidden devices.
    assert_eq!(devices.len(), 3);
    assert_eq!(devices[0].busid, "11-2");
    let visible = device_filter::visible_devices(&devices, &rules, false);
    assert_eq!(visible.len(), 1);
    // Row 0 in the filtered view is not devices[0]; operations must use the stable id.
    assert_eq!(visible[0].busid, "10-1");
    assert_ne!(visible[0].busid, devices[0].busid);
    assert_eq!(visible[0].persisted_guid.as_deref(), Some("guid-10-1"));
    let raw_ids: Vec<_> = devices.iter().map(|d| d.busid.as_str()).collect();
    assert_eq!(raw_ids, ["11-2", "10-1", "9-1"]);
}

#[test]
fn validate_rejects_enabled_incomplete_rules_but_allows_disabled() {
    assert_eq!(device_filter::validate_rules(&[]), Ok(()));
    assert_eq!(
        device_filter::validate_rules(&[rule(FilterField::DescriptionContains, "x", true)]),
        Ok(())
    );
    assert_eq!(
        device_filter::validate_rules(&[rule(FilterField::DescriptionContains, "  ", true)]),
        Err(RuleError::EmptyValue)
    );
    assert_eq!(
        device_filter::validate_rules(&[rule(FilterField::VidPid, "12D1:ZZZZ", true)]),
        Err(RuleError::InvalidVidPid)
    );
    assert_eq!(
        device_filter::validate_rules(&[rule(FilterField::VidPid, "", false)]),
        Ok(())
    );
}

#[test]
fn persist_roundtrip_and_missing_key_defaults_to_empty() {
    let old = Config::from_json_str(r#"{"language":"zh_CN","extra":1}"#);
    assert!(device_filter::load_rules(&old).is_empty());
    assert_eq!(old.language(), "zh_CN");
    assert_eq!(old.get_json("extra").and_then(|v| v.as_i64()), Some(1));

    let mut cfg = old.clone();
    let rules = vec![
        rule(FilterField::DescriptionContains, "Camera", true),
        rule(FilterField::VidPid, "12D1:107E", false),
    ];
    device_filter::store_rules(&mut cfg, &rules);
    assert_eq!(device_filter::load_rules(&cfg), rules);
    // Unknown keys and other settings survive a filter save.
    assert_eq!(cfg.language(), "zh_CN");
    assert_eq!(cfg.get_json("extra").and_then(|v| v.as_i64()), Some(1));

    // Cancel: a discarded draft is not stored.
    let draft = vec![rule(FilterField::BusId, "11-2", true)];
    assert_ne!(draft, device_filter::load_rules(&cfg));
    let reloaded = device_filter::load_rules(&cfg);
    assert_eq!(reloaded, rules);

    // Malformed array entries are skipped; a non-array value hides nothing.
    let skip = Config::from_json_str(
        r#"{"device_filter_rules":[{"field":"nope","value":"x"},{"field":"bus_id","value":"1-1","enabled":true}]}"#,
    );
    let loaded = device_filter::load_rules(&skip);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].field, FilterField::BusId);
    assert_eq!(loaded[0].value, "1-1");

    let bad = Config::from_json_str(r#"{"device_filter_rules":{"oops":true}}"#);
    assert!(device_filter::load_rules(&bad).is_empty());

    // Shape written to JSON is stable and explicit.
    let stored = cfg.get_json(CONFIG_KEY).cloned().unwrap();
    assert_eq!(
        stored,
        json!([
            {"enabled": true, "field": "description_contains", "value": "Camera"},
            {"enabled": false, "field": "vid_pid", "value": "12D1:107E"}
        ])
    );
}
