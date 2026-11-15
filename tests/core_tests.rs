//! 核心层单元测试：解析器（state JSON / list 文本 / wsl 列表）、解码、命令映射、配置、i18n。
//! fixtures 来自 usbipd 5.3.0 在真实机器上的输出。

use usbipd_tool::core::actions::{attach_args, bind_args, bind_needs_force_hint, detach_args, unbind_args};
use usbipd_tool::core::config::Config;
use usbipd_tool::core::decode::decode_output;
use usbipd_tool::core::i18n::{self, I18n};
use usbipd_tool::core::models::DeviceState;
use usbipd_tool::core::parser::*;

const STATE_REAL: &str = include_str!("fixtures/state_5_3.json");
const LIST_REAL: &str = include_str!("fixtures/list_5_3.txt");

fn utf16le(s: &str, bom: bool) -> Vec<u8> {
    let mut v = if bom { vec![0xFF, 0xFE] } else { vec![] };
    for u in s.encode_utf16() {
        v.extend_from_slice(&u.to_le_bytes());
    }
    v
}

#[test]
fn state_json_real_output() {
    let devs = parse_state_json(STATE_REAL).unwrap();
    assert_eq!(devs.len(), 4);
    let ch340 = &devs[0];
    assert_eq!(ch340.busid, "6-1");
    assert_eq!(ch340.vidpid(), "1a86:7523");
    assert_eq!(ch340.description, "USB-SERIAL CH340 (COM4)");
    assert_eq!(ch340.state, DeviceState::Shared);
    assert_eq!(ch340.persisted_guid.as_deref(), Some("923960de-4765-4a17-bf9a-0c60dfc9d758"));
    assert_eq!(devs[1].state, DeviceState::NotShared);
    assert_eq!(devs[1].description, "USB 输入设备");
    assert_eq!(devs[3].vidpid(), "8087:0032");
    assert_eq!(devs[3].description, "英特尔(R) 无线 Bluetooth(R)");
}

#[test]
fn state_json_attached_and_edge_cases() {
    let text = r#"{"Devices":[
        {"BusId":"2-3","ClientIPAddress":"172.20.1.2","Description":"CP2102","InstanceId":"USB\\vid_10c4&pid_ea60\\0001","PersistedGuid":"abc"},
        {"BusId":"","Description":"no busid"},
        {"BusId":"4-1","ClientIPAddress":"  ","Description":null,"InstanceId":null,"PersistedGuid":"  "}
    ]}"#;
    let devs = parse_state_json(text).unwrap();
    assert_eq!(devs.len(), 2);
    assert_eq!(devs[0].state, DeviceState::Attached);
    assert_eq!(devs[0].client_ip.as_deref(), Some("172.20.1.2"));
    assert_eq!(devs[0].vidpid(), "10c4:ea60");
    assert_eq!(devs[1].state, DeviceState::NotShared);
    assert_eq!(devs[1].vidpid(), "");
    assert!(parse_state_json("{}").unwrap().is_empty());
    assert!(parse_state_json(r#"{"Devices":null}"#).unwrap().is_empty());
    assert!(parse_state_json("not json").is_err());
    // 带 BOM
    assert_eq!(parse_state_json(&format!("\u{feff}{STATE_REAL}")).unwrap().len(), 4);
}

#[test]
fn list_text_real_output() {
    let devs = parse_list_text(LIST_REAL);
    assert_eq!(devs.len(), 4);
    assert_eq!(devs[0].busid, "1-5");
    assert_eq!(devs[0].vidpid(), "8087:0032");
    assert_eq!(devs[0].description, "英特尔(R) 无线 Bluetooth(R)");
    assert_eq!(devs[0].state, DeviceState::NotShared);
    assert_eq!(devs[1].busid, "6-1");
    assert_eq!(devs[1].description, "USB-SERIAL CH340 (COM4)");
    assert_eq!(devs[1].state, DeviceState::Shared);
    assert_eq!(devs[3].busid, "10-1");
    assert_eq!(devs[3].vidpid(), "25a7:fa61");
}

#[test]
fn list_text_attached_crlf_and_persisted() {
    let text = "Connected:\r\nBUSID  VID:PID    DEVICE                    STATE\r\n2-3    10C4:EA60  CP210x USB to UART, COM5  Attached\r\n\r\nPersisted:\r\nGUID  DEVICE\r\n9-9    1111:2222  should not parse  Shared\r\n";
    let devs = parse_list_text(text);
    assert_eq!(devs.len(), 1);
    assert_eq!(devs[0].vidpid(), "10c4:ea60");
    assert_eq!(devs[0].description, "CP210x USB to UART, COM5");
    assert_eq!(devs[0].state, DeviceState::Attached);
    assert!(parse_list_text("").is_empty());
    assert!(parse_list_text("1-1 1234:5678 x Shared").is_empty()); // 无 Connected: 段
}

#[test]
fn wsl_list_utf16le_and_utf8() {
    // wsl.exe 传统输出：UTF-16LE（可能无 BOM）
    let quiet = utf16le("Ubuntu\r\nDebian\r\ndocker-desktop\r\n", false);
    let text = decode_output(&quiet);
    assert_eq!(parse_wsl_distros_quiet(&text), vec!["Ubuntu", "Debian", "docker-desktop"]);

    let verbose = utf16le("  NAME      STATE           VERSION\r\n* Ubuntu    Running         2\r\n  Debian    Stopped         2\r\n", true);
    let vt = decode_output(&verbose);
    assert_eq!(parse_wsl_default_distro(&vt).as_deref(), Some("Ubuntu"));

    // 本机实际输出（WSL_UTF8=1 / 新版 wsl）：UTF-8
    let real = b"  NAME      STATE           VERSION\r\n* Ubuntu    Running         2\r\n";
        assert_eq!(parse_wsl_distros_verbose(&vt), vec!["Ubuntu", "Debian"]);
    assert_eq!(parse_wsl_distros_verbose(&decode_output(real)), vec!["Ubuntu"]);
    assert!(parse_wsl_distros_verbose("  NAME      STATE           VERSION
").is_empty());

assert_eq!(parse_wsl_default_distro(&decode_output(real)).as_deref(), Some("Ubuntu"));
    assert_eq!(parse_wsl_default_distro("  NAME STATE\n  Ubuntu Running 2\n"), None);
    assert!(parse_wsl_distros_quiet("\u{feff}\r\n\r\n").is_empty());
}

#[test]
fn decode_fallbacks() {
    assert_eq!(decode_output(b""), "");
    assert_eq!(decode_output("已绑定设备".as_bytes()), "已绑定设备");
    assert_eq!(decode_output(b"\xEF\xBB\xBFhello"), "hello");
    // GBK 编码的「输入设备」
    let (gbk, _, _) = encoding_rs::GBK.encode("USB 输入设备");
    assert_eq!(decode_output(&gbk), "USB 输入设备");
    // UTF-16 BE BOM
    let be: Vec<u8> = [0xFE, 0xFF, 0x00, b'o', 0x00, b'k'].to_vec();
    assert_eq!(decode_output(&be), "ok");
}

#[test]
fn command_mapping_matches_readme() {
    assert_eq!(bind_args("1-2", false), ["bind", "--busid", "1-2"]);
    assert_eq!(bind_args("1-2", true), ["bind", "--busid", "1-2", "--force"]);
    assert_eq!(unbind_args("1-2"), ["unbind", "--busid", "1-2"]);
    assert_eq!(detach_args("1-2"), ["detach", "--busid", "1-2"]);
    assert_eq!(attach_args("1-2", None, None, false), ["attach", "--wsl", "--busid", "1-2"]);
    assert_eq!(attach_args("1-2", Some("Ubuntu"), None, false), ["attach", "--wsl", "Ubuntu", "--busid", "1-2"]);
    assert_eq!(
        attach_args("1-2", Some("Ubuntu"), Some("172.20.0.1"), true),
        ["attach", "--wsl", "Ubuntu", "--busid", "1-2", "--host-ip", "172.20.0.1", "--auto-attach"]
    );
    assert_eq!(attach_args("1-2", Some("  "), Some(""), false), ["attach", "--wsl", "--busid", "1-2"]);
    assert_eq!(
        usbipd_tool::core::usbipd::display_command(&attach_args("1-2", Some("My Distro"), None, false)),
        "usbipd attach --wsl \"My Distro\" --busid 1-2"
    );
    assert!(bind_needs_force_hint("usbipd: error: Device is not shared; it is incompatible with USBPcap"));
    assert!(bind_needs_force_hint("Use --force to override"));
    assert!(!bind_needs_force_hint("Access denied"));
}

#[test]
fn version_parse() {
    let v = "5.3.0-54+Branch.master.Sha.aa3db8b82c4cb5071fd31bc54211606c70886912.aa3db8b82c4cb5071fd31bc54211606c70886912\r\n";
    assert_eq!(parse_version(v).as_deref(), Some("5.3.0-54"));
    assert_eq!(parse_version("4.4.0\n").as_deref(), Some("4.4.0"));
    assert_eq!(parse_version("  "), None);
}

#[test]
fn config_keys_defaults_and_roundtrip() {
    let c = Config::default();
    let v: serde_json::Value = serde_json::from_str(&c.to_json_string()).unwrap();
    for k in ["language", "theme", "force_bind_default", "last_wsl_distro", "last_host_ip", "auto_attach_default"] {
        assert!(v.get(k).is_some(), "missing key {k}");
    }
    assert_eq!(c.language(), "");
    assert!(!c.force_bind_default());

    let mut c = Config::from_json_str(r#"{"language":"en_US","force_bind_default":true,"extra":1}"#);
    assert_eq!(c.language(), "en_US");
    assert!(c.force_bind_default());
    assert_eq!(c.theme(), "");
    c.set_str("last_wsl_distro", "Ubuntu");
    c.set_bool("auto_attach_default", true);
    let dir = std::env::temp_dir().join(format!("usbipdtool-test-{}", std::process::id()));
    let p = dir.join("config.json");
    c.save_to(&p).unwrap();
    let c2 = Config::load_from(&p);
    assert_eq!(c2, c);
    assert_eq!(c2.last_wsl_distro(), "Ubuntu");
    assert!(c2.auto_attach_default());
    let _ = std::fs::remove_dir_all(&dir);
    // 损坏的文件 → 默认值
    assert_eq!(Config::from_json_str("{broken"), Config::default());
}

#[test]
fn i18n_dictionaries_complete() {
    let zh: serde_json::Map<String, serde_json::Value> = serde_json::from_str(i18n::raw_dict("zh_CN")).unwrap();
    let en: serde_json::Map<String, serde_json::Value> = serde_json::from_str(i18n::raw_dict("en_US")).unwrap();
    let mut zk: Vec<_> = zh.keys().collect();
    let mut ek: Vec<_> = en.keys().collect();
    zk.sort();
    ek.sort();
    assert_eq!(zk, ek, "zh_CN / en_US keys differ");

    let t = I18n::new("en_US");
    assert_eq!(t.tr("btn.bind"), "Bind");
    assert_eq!(t.trf("msg.bind_ok", &[("busid", "1-2")]), "Bound device 1-2");
    assert_eq!(t.tr("no.such.key"), "no.such.key");
    let z = I18n::new("fr_FR");
    assert_eq!(z.lang(), "zh_CN");
    assert_eq!(z.trf("status.usbipd_ready", &[("version", "5.3.0")]), "usbipd 5.3.0 已就绪");
    assert_eq!(i18n::lang_from_locale("zh-CN"), "zh_CN");
    assert_eq!(i18n::lang_from_locale("en_US.UTF-8"), "en_US");
}


#[test]
fn crate_version() {
    let v = usbipd_tool::VERSION;
    assert!(!v.is_empty());
    let core = v.split(['-', '+']).next().unwrap();
    let parts: Vec<_> = core.split('.').collect();
    assert!(parts.len() >= 3, "expected semver, got {v}");
    assert!(parts.iter().take(3).all(|p| p.chars().all(|c| c.is_ascii_digit())), "{v}");
}
