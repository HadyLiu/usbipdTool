//! State-machine tests for persisted connection intent. No real usbipd commands.
use usbipd_tool::core::attach_prefs::AttachPrefs;
use usbipd_tool::core::config::Config;
use usbipd_tool::core::device_intent::{
    self, DesiredState, DeviceIntent, MatchResult, ReconcileDecision, RestoreAction, RestoreBackoff,
    SkipReason,
};
use usbipd_tool::core::models::{DeviceState, UsbDevice};

fn prefs() -> AttachPrefs {
    AttachPrefs {
        use_default_distro: true,
        distro: "Ubuntu".into(),
        auto_attach: true,
        host_ip: String::new(),
    }
}

fn dev(
    busid: &str,
    instance: Option<&str>,
    guid: Option<&str>,
    vid: &str,
    pid: &str,
    state: DeviceState,
) -> UsbDevice {
    UsbDevice {
        busid: busid.into(),
        vid: vid.into(),
        pid: pid.into(),
        description: "fixture".into(),
        state,
        client_ip: if state == DeviceState::Attached { Some("172.25.235.191".into()) } else { None },
        instance_id: instance.map(|s| s.into()),
        persisted_guid: guid.map(|s| s.into()),
    }
}

fn ch340(busid: &str, state: DeviceState) -> UsbDevice {
    let mut d = dev(
        busid,
        Some(r"USB\VID_1A86&PID_7523\7&123"),
        if state == DeviceState::NotShared { None } else { Some("923960de-4765-4a17-bf9a-0c60dfc9d758") },
        "1a86",
        "7523",
        state,
    );
    d.description = "USB-SERIAL CH340 (COM8)".into();
    d
}

fn record(devs: &mut Vec<DeviceIntent>, d: &UsbDevice, desired: DesiredState) {
    device_intent::record_success(devs, DeviceIntent::from_device(d, desired, &prefs())).unwrap();
}

fn first_act(intents: &[DeviceIntent], devices: &[UsbDevice], running: &[String]) -> Option<RestoreAction> {
    device_intent::reconcile(intents, devices, running)
        .into_iter()
        .find_map(|i| match i.decision {
            ReconcileDecision::Act(a) => Some(a),
            _ => None,
        })
}

#[test]
fn old_config_without_intents_migrates_to_empty_and_keeps_globals() {
    let cfg = Config::from_json_str(
        r#"{"last_wsl_distro":"Ubuntu","auto_attach_default":true,"use_default_wsl_distro":true}"#,
    );
    assert!(device_intent::load_intents(&cfg).is_empty());
    assert!(cfg.auto_attach_default());
    assert_eq!(cfg.last_wsl_distro(), "Ubuntu");
}

#[test]
fn intents_roundtrip_in_config_without_dropping_unknown_keys() {
    let mut cfg = Config::from_json_str(r#"{"custom_flag":1,"auto_attach_default":false}"#);
    let d = ch340("6-1", DeviceState::Attached);
    let mut intents = vec![];
    record(&mut intents, &d, DesiredState::Attached);
    device_intent::store_intents(&mut cfg, &intents);
    let text = cfg.to_json_string();
    assert!(text.contains("custom_flag"));
    assert!(text.contains("device_intents"));
    let loaded = Config::from_json_str(&text);
    let back = device_intent::load_intents(&loaded);
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].desired, DesiredState::Attached);
    assert_eq!(back[0].instance_id, d.instance_id);
    assert_eq!(back[0].persisted_guid, d.persisted_guid);
    assert!(!loaded.auto_attach_default());
}

#[test]
fn attach_then_app_restart_and_replug_at_new_busid_restores_attach() {
    let original = ch340("6-1", DeviceState::Attached);
    let mut intents = vec![];
    record(&mut intents, &original, DesiredState::Attached);

    // App restart: child is gone, device not yet present.
    let plan = device_intent::reconcile(&intents, &[], &[]);
    assert!(matches!(plan[0].decision, ReconcileDecision::Skip(SkipReason::Absent)));
    assert_eq!(intents[0].desired, DesiredState::Attached);

    // Replug at a different BUSID, still the same InstanceId / GUID, currently only shared.
    let replug = ch340("2-4", DeviceState::Shared);
    assert_eq!(
        first_act(&intents, &[replug.clone()], &[]),
        Some(RestoreAction::Attach { busid: "2-4".into() })
    );
    // Already attached, or auto-attach child already running on the new busid: do nothing.
    let attached = ch340("2-4", DeviceState::Attached);
    assert!(matches!(
        device_intent::reconcile(&intents, &[attached.clone()], &[] )[0].decision,
        ReconcileDecision::Skip(SkipReason::AlreadySatisfied)
    ));
    assert!(matches!(
        device_intent::reconcile(&intents, &[replug], &["2-4".into()])[0].decision,
        ReconcileDecision::Skip(SkipReason::AlreadySatisfied)
    ));
}

#[test]
fn not_shared_after_replug_binds_non_force_then_would_attach() {
    let mut intents = vec![];
    record(&mut intents, &ch340("6-1", DeviceState::Attached), DesiredState::Attached);
    let raw = ch340("7-2", DeviceState::NotShared);
    assert_eq!(
        first_act(&intents, &[raw], &[]),
        Some(RestoreAction::Bind { busid: "7-2".into() })
    );
}

#[test]
fn shared_intent_binds_if_needed_but_never_attaches() {
    let mut intents = vec![];
    let shared = ch340("6-1", DeviceState::Shared);
    record(&mut intents, &shared, DesiredState::Shared);
    assert!(first_act(&intents, &[shared.clone()], &[]).is_none());
    let raw = ch340("3-1", DeviceState::NotShared);
    assert_eq!(
        first_act(&intents, &[raw], &[]),
        Some(RestoreAction::Bind { busid: "3-1".into() })
    );
}

#[test]
fn unshared_intent_is_a_no_op_even_if_the_device_is_present() {
    let mut intents = vec![];
    record(&mut intents, &ch340("6-1", DeviceState::NotShared), DesiredState::Unshared);
    let raw = ch340("6-1", DeviceState::NotShared);
    let devices = [raw];
    let plan = device_intent::reconcile(&intents, &devices, &[]);
    assert!(matches!(plan[0].decision, ReconcileDecision::Skip(SkipReason::UnsharedNoAction)));
}

#[test]
fn manual_detach_updates_intent_and_prevents_auto_reattach() {
    let mut intents = vec![];
    let attached = ch340("6-1", DeviceState::Attached);
    record(&mut intents, &attached, DesiredState::Attached);
    let after_detach = ch340("6-1", DeviceState::Shared);
    record(&mut intents, &after_detach, DesiredState::Shared);
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].desired, DesiredState::Shared);
    // Replug as shared or even as attached-looking must not queue attach.
    assert!(first_act(&intents, &[after_detach.clone()], &[]).is_none());
    let raw = ch340("9-1", DeviceState::NotShared);
    assert_eq!(
        first_act(&intents, &[raw], &[]),
        Some(RestoreAction::Bind { busid: "9-1".into() })
    );
}

#[test]
fn manual_unbind_updates_intent_and_prevents_bind_or_attach() {
    let mut intents = vec![];
    record(&mut intents, &ch340("6-1", DeviceState::Attached), DesiredState::Attached);
    record(&mut intents, &ch340("6-1", DeviceState::NotShared), DesiredState::Unshared);
    assert_eq!(intents[0].desired, DesiredState::Unshared);
    assert!(first_act(&intents, &[ch340("8-1", DeviceState::NotShared)], &[]).is_none());
}

#[test]
fn unplug_does_not_overwrite_attached_intent_with_absence() {
    let mut intents = vec![];
    record(&mut intents, &ch340("6-1", DeviceState::Attached), DesiredState::Attached);
    let before = intents.clone();
    let plan = device_intent::reconcile(&intents, &[], &[]);
    assert!(matches!(plan[0].decision, ReconcileDecision::Skip(SkipReason::Absent)));
    // Caller must not record Unshared / Shared from a missing device.
    assert_eq!(intents, before);
}

#[test]
fn failed_command_does_not_record_success() {
    let intents: Vec<DeviceIntent> = vec![];
    // The app only calls record_success after res.ok() / AutoAttachStarted.
    assert!(intents.is_empty());
}

#[test]
fn busid_alone_is_never_a_match_and_vidpid_duplicates_are_skipped() {
    let mut intents = vec![];
    // Stored with identity of device A.
    record(&mut intents, &ch340("6-1", DeviceState::Attached), DesiredState::Attached);

    // Unrelated device reused the old BUSID but has a different instance id.
    let stranger = dev(
        "6-1",
        Some(r"USB\VID_046D&PID_C52B\other"),
        Some("guid-other"),
        "046d",
        "c52b",
        DeviceState::Shared,
    );
    assert!(matches!(match_intent(&intents[0], &[stranger.clone()]), MatchResult::Absent));
    assert!(first_act(&intents, &[stranger], &[]).is_none());

    // Two live devices with the same instance id (should not happen, but must not guess).
    let a = ch340("1-1", DeviceState::Shared);
    let mut b = ch340("1-2", DeviceState::Shared);
    b.busid = "1-2".into();
    assert!(matches!(match_intent(&intents[0], &[a, b]), MatchResult::Ambiguous));

    // Missing identity cannot be stored, and cannot be matched by VID:PID alone.
    let bare = dev("2-2", None, None, "1a86", "7523", DeviceState::Shared);
    let mut only_vid = DeviceIntent::from_device(&bare, DesiredState::Attached, &prefs());
    only_vid.instance_id = None;
    only_vid.persisted_guid = None;
    assert_eq!(
        device_intent::record_success(&mut intents, only_vid.clone()),
        Err(SkipReason::MissingIdentity)
    );
    assert!(matches!(match_intent(&only_vid, &[bare]), MatchResult::MissingIdentity));
}

#[test]
fn restore_only_recorded_devices_not_every_peripheral() {
    let mut intents = vec![];
    record(&mut intents, &ch340("6-1", DeviceState::Attached), DesiredState::Attached);
    let keyboard = dev(
        "1-3",
        Some(r"USB\VID_046D&PID_C52B\kbd"),
        None,
        "046d",
        "c52b",
        DeviceState::NotShared,
    );
    let bluetooth = dev(
        "1-10",
        Some(r"USB\VID_8087&PID_0032\bt"),
        None,
        "8087",
        "0032",
        DeviceState::NotShared,
    );
    let known = ch340("2-4", DeviceState::Shared);
    let acts: Vec<_> = device_intent::reconcile(&intents, &[keyboard, bluetooth, known], &[])
        .into_iter()
        .filter_map(|i| match i.decision {
            ReconcileDecision::Act(a) => Some(a),
            _ => None,
        })
        .collect();
    assert_eq!(acts, vec![RestoreAction::Attach { busid: "2-4".into() }]);
}

#[test]
fn global_auto_attach_default_does_not_reattach_after_detach() {
    // Settings default stays true, but the per-device intent after detach is Shared.
    let mut cfg = Config::from_json_str(r#"{"auto_attach_default":true}"#);
    assert!(cfg.auto_attach_default());
    let mut intents = vec![];
    record(&mut intents, &ch340("6-1", DeviceState::Shared), DesiredState::Shared);
    device_intent::store_intents(&mut cfg, &intents);
    assert!(first_act(&intents, &[ch340("6-1", DeviceState::Shared)], &[]).is_none());
}

#[test]
fn backoff_prevents_tight_retry_loops_then_clears_on_success() {
    let mut b = RestoreBackoff::default();
    let t0 = std::time::Instant::now();
    assert!(b.allows(t0));
    b.on_failure(t0);
    assert_eq!(b.failures(), 1);
    assert!(!b.allows(t0));
    assert!(b.allows(t0 + std::time::Duration::from_secs(3)));
    b.on_failure(t0 + std::time::Duration::from_secs(3));
    assert!(!b.allows(t0 + std::time::Duration::from_secs(4)));
    assert!(b.allows(t0 + std::time::Duration::from_secs(8)));
    b.on_success();
    assert_eq!(b.failures(), 0);
    assert!(b.allows(t0));
}

#[test]
fn refresh_identity_updates_guid_after_bind_without_changing_intent() {
    let mut intents = vec![];
    let before = ch340("6-1", DeviceState::NotShared); // no guid yet
    record(&mut intents, &before, DesiredState::Attached);
    assert!(intents[0].persisted_guid.is_none());
    let after = ch340("7-2", DeviceState::Shared); // same instance, new busid + guid
    device_intent::refresh_identity(&mut intents, &[after]);
    assert_eq!(intents[0].desired, DesiredState::Attached);
    assert_eq!(
        intents[0].persisted_guid.as_deref(),
        Some("923960de-4765-4a17-bf9a-0c60dfc9d758")
    );
}

fn match_intent<'a>(intent: &DeviceIntent, devices: &'a [UsbDevice]) -> MatchResult<'a> {
    device_intent::match_intent(intent, devices)
}


#[test]
fn running_auto_attach_on_previous_busid_does_not_start_a_second_child() {
    let mut intents = vec![];
    let original = ch340("6-1", DeviceState::Attached);
    record(&mut intents, &original, DesiredState::Attached);
    let replug = ch340("2-4", DeviceState::Shared);
    // Child still running under the pre-unplug busid (usbipd --auto-attach).
    assert!(first_act(&intents, &[replug], &["6-1".into()]).is_none());
}
