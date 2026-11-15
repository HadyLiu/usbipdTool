//! Attach preference persistence, validation, cancel, and direct-attach args.
use usbipd_tool::core::attach_prefs::{is_valid_host_ip, AttachPrefs, PrefsError};
use usbipd_tool::core::config::Config;
use usbipd_tool::core::usbipd::display_command;

fn prefs(use_default: bool, distro: &str, auto_attach: bool, host_ip: &str) -> AttachPrefs {
    AttachPrefs {
        use_default_distro: use_default,
        distro: distro.to_string(),
        auto_attach,
        host_ip: host_ip.to_string(),
    }
}

#[test]
fn missing_use_default_key_migrates_to_true_and_keeps_remembered_distro() {
    let cfg = Config::from_json_str(r#"{"last_wsl_distro":"Ubuntu","last_host_ip":"172.20.0.1","auto_attach_default":true}"#);
    assert!(cfg.use_default_wsl_distro());
    assert_eq!(cfg.last_wsl_distro(), "Ubuntu");
    let p = AttachPrefs::from_config(&cfg);
    assert!(p.use_default_distro);
    assert_eq!(p.distro, "Ubuntu");
    assert_eq!(p.distribution(), None);
    assert_eq!(
        p.attach_args("7-1").unwrap(),
        ["attach", "--wsl", "--busid", "7-1", "--host-ip", "172.20.0.1", "--auto-attach"]
    );
}

#[test]
fn saving_default_distribution_does_not_clear_remembered_distro() {
    let mut cfg = Config::from_json_str(r#"{"last_wsl_distro":"Ubuntu","use_default_wsl_distro":false}"#);
    assert!(!cfg.use_default_wsl_distro());
    let mut draft = AttachPrefs::from_config(&cfg);
    draft.use_default_distro = true;
    draft.auto_attach = true;
    draft.host_ip = " 10.0.0.2 ".into();
    draft.apply_to_config(&mut cfg);
    assert!(cfg.use_default_wsl_distro());
    assert_eq!(cfg.last_wsl_distro(), "Ubuntu");
    assert_eq!(cfg.last_host_ip(), "10.0.0.2");
    assert!(cfg.auto_attach_default());
    assert_eq!(AttachPrefs::from_config(&cfg).distribution(), None);
}

#[test]
fn cancel_discards_draft_edits() {
    let cfg = Config::from_json_str(
        r#"{"last_wsl_distro":"Ubuntu","use_default_wsl_distro":false,"last_host_ip":"","auto_attach_default":false}"#,
    );
    let before = AttachPrefs::from_config(&cfg);
    let mut draft = before.clone();
    draft.use_default_distro = true;
    draft.distro = "Debian".into();
    draft.auto_attach = true;
    draft.host_ip = "192.168.1.1".into();
    // Cancel: drop the draft without apply_to_config.
    drop(draft);
    assert_eq!(AttachPrefs::from_config(&cfg), before);
    assert_eq!(cfg.last_wsl_distro(), "Ubuntu");
    assert!(!cfg.use_default_wsl_distro());
    assert!(!cfg.auto_attach_default());
}

#[test]
fn explicit_distro_and_optional_ip_build_direct_attach_args() {
    let p = prefs(false, "Ubuntu", true, "172.20.0.1");
    assert_eq!(
        p.attach_args("1-5").unwrap(),
        ["attach", "--wsl", "Ubuntu", "--busid", "1-5", "--host-ip", "172.20.0.1", "--auto-attach"]
    );
    let preview = display_command(&p.preview_args());
    assert!(preview.contains("<busid>"), "{preview}");
    assert!(!preview.contains("1-5"), "{preview}");
}

#[test]
fn validate_distro_and_optional_ip() {
    assert!(prefs(true, "", false, "").validate().is_ok());
    assert!(prefs(true, "Ubuntu", false, "").attach_args("6-1").unwrap().eq(&["attach", "--wsl", "--busid", "6-1"]));
    assert_eq!(prefs(false, "  ", false, "").validate(), Err(PrefsError::DistroRequired));
    assert_eq!(prefs(false, "", false, "10.0.0.1").validate(), Err(PrefsError::DistroRequired));
    assert_eq!(prefs(true, "", false, "not-an-ip").validate(), Err(PrefsError::InvalidHostIp));
    assert_eq!(prefs(false, "Ubuntu", false, "172.20.0.1:22").validate(), Err(PrefsError::InvalidHostIp));
    assert!(prefs(false, " Ubuntu ", false, "  ::1  ").validate().is_ok());
    assert!(is_valid_host_ip("192.168.0.1"));
    assert!(is_valid_host_ip("::1"));
    assert!(!is_valid_host_ip("localhost"));
    assert!(!is_valid_host_ip(""));
}

#[test]
fn new_config_key_defaults_true_and_roundtrips() {
    let c = Config::default();
    assert!(c.use_default_wsl_distro());
    let v: serde_json::Value = serde_json::from_str(&c.to_json_string()).unwrap();
    assert_eq!(v["use_default_wsl_distro"], true);
    let mut c = Config::default();
    let p = prefs(false, "Debian", false, "");
    p.apply_to_config(&mut c);
    let dir = std::env::temp_dir().join(format!("usbipdtool-prefs-{}", std::process::id()));
    let path = dir.join("config.json");
    c.save_to(&path).unwrap();
    let loaded = Config::load_from(&path);
    assert!(!loaded.use_default_wsl_distro());
    assert_eq!(loaded.last_wsl_distro(), "Debian");
    let _ = std::fs::remove_dir_all(&dir);
}


use usbipd_tool::core::attach_prefs::{default_distro_name, fill_blank_or_invalid_draft};
use usbipd_tool::core::models::WslDistro;

fn distros(pairs: &[(&str, bool)]) -> Vec<WslDistro> {
    pairs.iter().map(|(n, d)| WslDistro { name: n.to_string(), is_default: *d }).collect()
}

#[test]
fn detected_default_fills_blank_draft_but_does_not_save() {
    let list = distros(&[("Debian", false), ("Ubuntu", true)]);
    assert_eq!(default_distro_name(&list), Some("Ubuntu"));
    assert_eq!(fill_blank_or_invalid_draft("", &list).as_deref(), Some("Ubuntu"));
    assert_eq!(fill_blank_or_invalid_draft("  ", &list).as_deref(), Some("Ubuntu"));
    let cfg = Config::from_json_str(r#"{"use_default_wsl_distro":false,"last_wsl_distro":""}"#);
    let mut draft = AttachPrefs::from_config(&cfg);
    assert!(draft.distro.trim().is_empty());
    draft.distro = fill_blank_or_invalid_draft(&draft.distro, &list).unwrap();
    assert_eq!(draft.distro, "Ubuntu");
    // Filling the draft must not write the config until Confirm/Save.
    assert_eq!(cfg.last_wsl_distro(), "");
    assert!(!cfg.use_default_wsl_distro());
}

#[test]
fn explicit_valid_selection_is_preserved() {
    let list = distros(&[("Ubuntu", true), ("Debian", false)]);
    assert_eq!(fill_blank_or_invalid_draft("Debian", &list), None);
    assert_eq!(fill_blank_or_invalid_draft("Ubuntu", &list), None);
    let cfg = Config::from_json_str(r#"{"use_default_wsl_distro":false,"last_wsl_distro":"Debian"}"#);
    let mut draft = AttachPrefs::from_config(&cfg);
    if let Some(name) = fill_blank_or_invalid_draft(&draft.distro, &list) {
        draft.distro = name;
    }
    assert_eq!(draft.distro, "Debian");
    assert_eq!(cfg.last_wsl_distro(), "Debian");
}

#[test]
fn missing_or_unknown_name_falls_back_to_detected_default() {
    let list = distros(&[("Ubuntu", true), ("Debian", false)]);
    assert_eq!(fill_blank_or_invalid_draft("OldGone", &list).as_deref(), Some("Ubuntu"));
    // No * marker: first installed name is the fallback.
    let no_star = distros(&[("Fedora", false), ("Ubuntu", false)]);
    assert_eq!(default_distro_name(&no_star), Some("Fedora"));
    assert_eq!(fill_blank_or_invalid_draft("", &no_star).as_deref(), Some("Fedora"));
    // Empty list or load failure: do not invent a name, and do not save.
    assert_eq!(fill_blank_or_invalid_draft("", &[]), None);
    assert_eq!(fill_blank_or_invalid_draft("Ubuntu", &[]), None);
    let cfg = Config::from_json_str(r#"{"last_wsl_distro":"OldGone"}"#);
    let before = AttachPrefs::from_config(&cfg);
    let mut draft = before.clone();
    if let Some(name) = fill_blank_or_invalid_draft(&draft.distro, &list) {
        draft.distro = name;
    }
    assert_eq!(draft.distro, "Ubuntu");
    drop(draft);
    assert_eq!(AttachPrefs::from_config(&cfg), before);
}
