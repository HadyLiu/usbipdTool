//! Persisted per-device connection intent.
//!
//! Identity prefers usbipd `PersistedGuid` and Windows `InstanceId` from `usbipd state`
//! JSON. BUSID is never used as a stable key (it can change or be reused). VID:PID
//! alone is not enough to attach when more than one device shares it; missing or
//! conflicting identity is skipped conservatively with a reason for the log.
//!
//! Desired state is written only after a successful explicit user action
//! (bind / attach / detach / unbind). Disappearance on unplug does not clear an
//! Attached or Shared intent. Failed commands do not record success. Restoring
//! never binds or attaches a device that has no recorded intent.

use super::attach_prefs::AttachPrefs;
use super::config::Config;
use super::models::UsbDevice;
use serde_json::{Map, Value};

pub const CONFIG_KEY: &str = "device_intents";

/// Last successful user-requested connection state for a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesiredState {
    /// After a successful unbind (or an explicit "do not share") — restore does nothing.
    Unshared,
    /// After a successful bind (or detach). Restore may non-force bind; never attach.
    Shared,
    /// After a successful attach. Restore may bind (non-force) then attach with `--auto-attach`.
    Attached,
}

impl DesiredState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unshared => "unshared",
            Self::Shared => "shared",
            Self::Attached => "attached",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "unshared" => Some(Self::Unshared),
            "shared" => Some(Self::Shared),
            "attached" => Some(Self::Attached),
            _ => None,
        }
    }
}

/// One remembered device. Distro / host / auto-attach flags are stored per device;
/// new attaches still fall back to the global Settings defaults when a field is empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceIntent {
    pub instance_id: Option<String>,
    pub persisted_guid: Option<String>,
    pub vid: String,
    pub pid: String,
    pub description: String,
    pub desired: DesiredState,
    /// Used for *user-initiated* attach. Restore of [`DesiredState::Attached`] always
    /// starts `--auto-attach` so the session stays attached while the app runs.
    pub auto_attach: bool,
    pub use_default_distro: bool,
    pub distro: String,
    pub host_ip: String,
    /// Last observed BUSID, used only to detect an already-running `--auto-attach` child
    /// after a busid change. Never used as a match key.
    pub last_busid: String,
}

impl DeviceIntent {
    pub fn from_device(dev: &UsbDevice, desired: DesiredState, prefs: &AttachPrefs) -> Self {
        Self {
            instance_id: non_empty_owned(dev.instance_id.as_deref()),
            persisted_guid: non_empty_owned(dev.persisted_guid.as_deref()),
            vid: dev.vid.clone(),
            pid: dev.pid.clone(),
            description: dev.description.clone(),
            desired,
            auto_attach: prefs.auto_attach,
            use_default_distro: prefs.use_default_distro,
            distro: prefs.distro.trim().to_string(),
            host_ip: prefs.host_ip.trim().to_string(),
            last_busid: dev.busid.clone(),
        }
    }

    pub fn has_stable_identity(&self) -> bool {
        self.instance_id.is_some() || self.persisted_guid.is_some()
    }

    pub fn attach_prefs(&self) -> AttachPrefs {
        AttachPrefs {
            use_default_distro: self.use_default_distro,
            distro: self.distro.clone(),
            // Restore of an attached intent always requests --auto-attach.
            auto_attach: true,
            host_ip: self.host_ip.clone(),
        }
    }
}

fn non_empty_owned(s: Option<&str>) -> Option<String> {
    s.map(str::trim).filter(|v| !v.is_empty()).map(str::to_string)
}

fn eq_ident(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Why a recorded intent will not be restored against the current list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// No PersistedGuid / InstanceId to match; VID:PID is not used alone.
    MissingIdentity,
    /// Two or more live devices share the stored GUID or instance id (or they disagree).
    Ambiguous,
    /// The recorded device is not in the current list (unplugged). Intent is kept as-is.
    Absent,
    /// Desired state is already satisfied (or a matching auto-attach child is running).
    AlreadySatisfied,
    /// Desired state is unshared — restore is a no-op.
    UnsharedNoAction,
}

impl SkipReason {
    pub fn log_message(&self, intent: &DeviceIntent) -> String {
        let name = if !intent.description.trim().is_empty() {
            intent.description.as_str()
        } else {
            "device"
        };
        match self {
            Self::MissingIdentity => format!(
                "[restore] skip {name}: no stable identity (PersistedGuid/InstanceId); will not match by VID:PID alone"
            ),
            Self::Ambiguous => format!(
                "[restore] skip {name}: ambiguous identity (duplicate PersistedGuid or InstanceId); will not guess"
            ),
            Self::Absent => format!("[restore] {name}: not present; keeping remembered state"),
            Self::AlreadySatisfied => format!("[restore] {name}: already in the remembered state"),
            Self::UnsharedNoAction => format!("[restore] {name}: remembered as not shared; no action"),
        }
    }
}

/// A unique match of a stored intent to a live device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchResult<'a> {
    Unique(&'a UsbDevice),
    Absent,
    Ambiguous,
    MissingIdentity,
}

/// Match one intent against the current scan. Prefers PersistedGuid, then InstanceId.
/// Does not fall back to BUSID or to a VID:PID-only match.
pub fn match_intent<'a>(intent: &DeviceIntent, devices: &'a [UsbDevice]) -> MatchResult<'a> {
    if !intent.has_stable_identity() {
        return MatchResult::MissingIdentity;
    }

    let by_guid: Vec<&UsbDevice> = intent
        .persisted_guid
        .as_deref()
        .map(|g| {
            devices
                .iter()
                .filter(|d| d.persisted_guid.as_deref().is_some_and(|x| eq_ident(x, g)))
                .collect()
        })
        .unwrap_or_default();

    let by_instance: Vec<&UsbDevice> = intent
        .instance_id
        .as_deref()
        .map(|id| {
            devices
                .iter()
                .filter(|d| d.instance_id.as_deref().is_some_and(|x| eq_ident(x, id)))
                .collect()
        })
        .unwrap_or_default();

    if by_guid.len() > 1 || by_instance.len() > 1 {
        return MatchResult::Ambiguous;
    }
    match (by_guid.first().copied(), by_instance.first().copied()) {
        (Some(a), Some(b)) if a.busid != b.busid => MatchResult::Ambiguous,
        (Some(a), _) => MatchResult::Unique(a),
        (None, Some(b)) => MatchResult::Unique(b),
        (None, None) => MatchResult::Absent,
    }
}

/// One restore step. Bind is never `--force`. Attach is only for Attached intents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreAction {
    Bind { busid: String },
    Attach { busid: String },
}

/// What reconcile decided for a single recorded intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileDecision {
    Act(RestoreAction),
    Skip(SkipReason),
}

pub struct ReconcileItem<'a> {
    pub intent: &'a DeviceIntent,
    pub decision: ReconcileDecision,
}

/// Compare remembered intents with a fresh device list and running `--auto-attach` busids.
/// At most the caller should execute the first `Act` (one at a time) to avoid busy races
/// and duplicate attach processes. Observed absence never mutates the stored intent.
pub fn reconcile<'a>(
    intents: &'a [DeviceIntent],
    devices: &'a [UsbDevice],
    running_auto_busids: &[String],
) -> Vec<ReconcileItem<'a>> {
    let mut out = Vec::new();
    for intent in intents {
        let decision = reconcile_one(intent, devices, running_auto_busids);
        out.push(ReconcileItem { intent, decision });
    }
    out
}

fn is_running(busid: &str, running: &[String]) -> bool {
    running.iter().any(|b| b == busid)
}

fn reconcile_one(
    intent: &DeviceIntent,
    devices: &[UsbDevice],
    running_auto_busids: &[String],
) -> ReconcileDecision {
    if intent.desired == DesiredState::Unshared {
        return ReconcileDecision::Skip(SkipReason::UnsharedNoAction);
    }
    match match_intent(intent, devices) {
        MatchResult::MissingIdentity => ReconcileDecision::Skip(SkipReason::MissingIdentity),
        MatchResult::Ambiguous => ReconcileDecision::Skip(SkipReason::Ambiguous),
        MatchResult::Absent => ReconcileDecision::Skip(SkipReason::Absent),
        MatchResult::Unique(dev) => match intent.desired {
            DesiredState::Unshared => ReconcileDecision::Skip(SkipReason::UnsharedNoAction),
            DesiredState::Shared => {
                if dev.is_shared() || dev.is_attached() {
                    ReconcileDecision::Skip(SkipReason::AlreadySatisfied)
                } else {
                    ReconcileDecision::Act(RestoreAction::Bind { busid: dev.busid.clone() })
                }
            }
            DesiredState::Attached => {
                if dev.is_attached()
                    || is_running(&dev.busid, running_auto_busids)
                    || (!intent.last_busid.is_empty() && is_running(&intent.last_busid, running_auto_busids))
                {
                    ReconcileDecision::Skip(SkipReason::AlreadySatisfied)
                } else if dev.is_not_shared() {
                    ReconcileDecision::Act(RestoreAction::Bind { busid: dev.busid.clone() })
                } else {
                    ReconcileDecision::Act(RestoreAction::Attach { busid: dev.busid.clone() })
                }
            }
        },
    }
}

/// Write or replace the intent for the same physical device after a successful user action.
/// Matching uses GUID / instance id; a new record is appended if none exists.
/// Entries without a stable identity are not stored (we would not be able to restore them safely).
pub fn record_success(intents: &mut Vec<DeviceIntent>, incoming: DeviceIntent) -> Result<(), SkipReason> {
    if !incoming.has_stable_identity() {
        return Err(SkipReason::MissingIdentity);
    }
    if let Some(existing) = find_mutable(intents, &incoming) {
        existing.persisted_guid = incoming.persisted_guid.or(existing.persisted_guid.take());
        existing.instance_id = incoming.instance_id.or(existing.instance_id.take());
        if !incoming.vid.is_empty() {
            existing.vid = incoming.vid;
        }
        if !incoming.pid.is_empty() {
            existing.pid = incoming.pid;
        }
        if !incoming.description.is_empty() {
            existing.description = incoming.description;
        }
        existing.desired = incoming.desired;
        existing.auto_attach = incoming.auto_attach;
        existing.use_default_distro = incoming.use_default_distro;
        existing.distro = incoming.distro;
        existing.host_ip = incoming.host_ip;
        if !incoming.last_busid.is_empty() {
            existing.last_busid = incoming.last_busid;
        }
        Ok(())
    } else {
        intents.push(incoming);
        Ok(())
    }
}

fn find_mutable<'a>(intents: &'a mut [DeviceIntent], incoming: &DeviceIntent) -> Option<&'a mut DeviceIntent> {
    let guid = incoming.persisted_guid.clone();
    let inst = incoming.instance_id.clone();
    intents.iter_mut().find(|e| {
        match (&e.persisted_guid, &guid) {
            (Some(a), Some(b)) if eq_ident(a, b) => return true,
            _ => {}
        }
        match (&e.instance_id, &inst) {
            (Some(a), Some(b)) if eq_ident(a, b) => return true,
            _ => {}
        }
        false
    })
}

/// Refresh stored GUID / description from a unique live match without changing desired state.
/// No-op when the device is absent or the match is ambiguous.
pub fn refresh_identity(intents: &mut [DeviceIntent], devices: &[UsbDevice]) {
    // Snapshot matches first to avoid aliasing issues while mutating.
    let updates: Vec<(usize, Option<String>, Option<String>, String, String, String, String)> = intents
        .iter()
        .enumerate()
        .filter_map(|(i, intent)| match match_intent(intent, devices) {
            MatchResult::Unique(dev) => Some((
                i,
                non_empty_owned(dev.persisted_guid.as_deref()),
                non_empty_owned(dev.instance_id.as_deref()),
                dev.vid.clone(),
                dev.pid.clone(),
                dev.description.clone(),
                dev.busid.clone(),
            )),
            _ => None,
        })
        .collect();
    for (i, guid, inst, vid, pid, desc, busid) in updates {
        if let Some(g) = guid {
            intents[i].persisted_guid = Some(g);
        }
        if let Some(id) = inst {
            intents[i].instance_id = Some(id);
        }
        if !vid.is_empty() {
            intents[i].vid = vid;
        }
        if !pid.is_empty() {
            intents[i].pid = pid;
        }
        if !desc.is_empty() {
            intents[i].description = desc;
        }
        if !busid.is_empty() {
            intents[i].last_busid = busid;
        }
    }
}

pub fn load_intents(cfg: &Config) -> Vec<DeviceIntent> {
    match cfg.get_json(CONFIG_KEY) {
        Some(Value::Array(items)) => items.iter().filter_map(from_json).collect(),
        _ => Vec::new(),
    }
}

pub fn store_intents(cfg: &mut Config, intents: &[DeviceIntent]) {
    let items: Vec<Value> = intents.iter().map(to_json).collect();
    cfg.set_json(CONFIG_KEY, Value::Array(items));
}

fn to_json(intent: &DeviceIntent) -> Value {
    let mut m = Map::new();
    if let Some(v) = &intent.instance_id {
        m.insert("instance_id".into(), Value::String(v.clone()));
    }
    if let Some(v) = &intent.persisted_guid {
        m.insert("persisted_guid".into(), Value::String(v.clone()));
    }
    m.insert("vid".into(), Value::String(intent.vid.clone()));
    m.insert("pid".into(), Value::String(intent.pid.clone()));
    m.insert("description".into(), Value::String(intent.description.clone()));
    m.insert("desired".into(), Value::String(intent.desired.as_str().into()));
    m.insert("auto_attach".into(), Value::Bool(intent.auto_attach));
    m.insert("use_default_distro".into(), Value::Bool(intent.use_default_distro));
    m.insert("distro".into(), Value::String(intent.distro.clone()));
    m.insert("host_ip".into(), Value::String(intent.host_ip.clone()));
    if !intent.last_busid.is_empty() {
        m.insert("last_busid".into(), Value::String(intent.last_busid.clone()));
    }
    Value::Object(m)
}

fn from_json(v: &Value) -> Option<DeviceIntent> {
    let o = v.as_object()?;
    let desired = o
        .get("desired")
        .and_then(Value::as_str)
        .and_then(DesiredState::parse)?;
    Some(DeviceIntent {
        instance_id: o.get("instance_id").and_then(Value::as_str).and_then(|s| non_empty_owned(Some(s))),
        persisted_guid: o.get("persisted_guid").and_then(Value::as_str).and_then(|s| non_empty_owned(Some(s))),
        vid: o.get("vid").and_then(Value::as_str).unwrap_or("").to_string(),
        pid: o.get("pid").and_then(Value::as_str).unwrap_or("").to_string(),
        description: o.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
        desired,
        auto_attach: o.get("auto_attach").and_then(Value::as_bool).unwrap_or(false),
        use_default_distro: o.get("use_default_distro").and_then(Value::as_bool).unwrap_or(true),
        distro: o.get("distro").and_then(Value::as_str).unwrap_or("").to_string(),
        host_ip: o.get("host_ip").and_then(Value::as_str).unwrap_or("").to_string(),
        last_busid: o.get("last_busid").and_then(Value::as_str).unwrap_or("").to_string(),
    })
}

/// In-memory backoff so a failed restore does not retry on every scan.
#[derive(Debug, Clone)]
pub struct RestoreBackoff {
    failures: u32,
    next_allowed: Option<std::time::Instant>,
}

impl Default for RestoreBackoff {
    fn default() -> Self {
        Self { failures: 0, next_allowed: None }
    }
}

impl RestoreBackoff {
    pub fn allows(&self, now: std::time::Instant) -> bool {
        self.next_allowed.is_none_or(|t| now >= t)
    }

    pub fn on_success(&mut self) {
        self.failures = 0;
        self.next_allowed = None;
    }

    pub fn on_failure(&mut self, now: std::time::Instant) {
        self.failures = self.failures.saturating_add(1).min(6);
        let secs = 2u64.saturating_pow(self.failures).min(60);
        self.next_allowed = Some(now + std::time::Duration::from_secs(secs));
    }

    pub fn failures(&self) -> u32 {
        self.failures
    }
}

pub fn backoff_key(intent: &DeviceIntent) -> String {
    if let Some(g) = &intent.persisted_guid {
        format!("guid:{g}")
    } else if let Some(id) = &intent.instance_id {
        format!("instance:{id}")
    } else {
        format!("vidpid:{}:{}", intent.vid, intent.pid)
    }
}
