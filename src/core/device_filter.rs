//! Display-only device hide rules. Persistence is backward-compatible: a missing
//! `device_filter_rules` key means no rules, so nothing is hidden.
//!
//! Matching is a simple field + value form (not regex): any enabled rule that
//! matches hides the device (OR). Empty rules, disabled rules, and empty values
//! never hide a device.

use crate::core::config::Config;
use crate::core::models::UsbDevice;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONFIG_KEY: &str = "device_filter_rules";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FilterField {
    #[default]
    DescriptionContains,
    VidPid,
    BusId,
}

impl FilterField {
    pub const ALL: [FilterField; 3] = [
        FilterField::DescriptionContains,
        FilterField::VidPid,
        FilterField::BusId,
    ];

    pub fn i18n_key(self) -> &'static str {
        match self {
            Self::DescriptionContains => "filter.field.description",
            Self::VidPid => "filter.field.vidpid",
            Self::BusId => "filter.field.busid",
        }
    }

    pub fn placeholder_key(self) -> &'static str {
        match self {
            Self::DescriptionContains => "filter.placeholder.description",
            Self::VidPid => "filter.placeholder.vidpid",
            Self::BusId => "filter.placeholder.busid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterRule {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub field: FilterField,
    #[serde(default)]
    pub value: String,
}

fn default_enabled() -> bool {
    true
}

impl FilterRule {
    pub fn new(field: FilterField) -> Self {
        Self { enabled: true, field, value: String::new() }
    }
}

impl Default for FilterRule {
    fn default() -> Self {
        Self::new(FilterField::DescriptionContains)
    }
}

/// Normalize and validate a VID:PID value. Accepts 1–4 hex digits on each side,
/// compared as 4-digit uppercase (so `12d1:7e` matches `12D1:007E`).
pub fn parse_vidpid(text: &str) -> Option<(String, String)> {
    let text = text.trim();
    let (vid, pid) = text.split_once(':')?;
    let vid = vid.trim();
    let pid = pid.trim();
    if vid.is_empty() || pid.is_empty() || vid.len() > 4 || pid.len() > 4 {
        return None;
    }
    if !vid.chars().all(|c| c.is_ascii_hexdigit()) || !pid.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some((format!("{vid:0>4}").to_ascii_uppercase(), format!("{pid:0>4}").to_ascii_uppercase()))
}

fn display_description(dev: &UsbDevice) -> &str {
    if !dev.description.is_empty() {
        &dev.description
    } else {
        dev.instance_id.as_deref().unwrap_or("")
    }
}

fn description_contains(dev: &UsbDevice, needle: &str) -> bool {
    let needle = needle.trim();
    if needle.is_empty() {
        return false;
    }
    display_description(dev)
        .to_lowercase()
        .contains(&needle.to_lowercase())
}

fn vidpid_exact(dev: &UsbDevice, value: &str) -> bool {
    let Some((vid, pid)) = parse_vidpid(value) else {
        return false;
    };
    let Some((dvid, dpid)) = parse_vidpid(&dev.vidpid()) else {
        return false;
    };
    vid == dvid && pid == dpid
}

fn busid_exact(dev: &UsbDevice, value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || dev.busid.trim().is_empty() {
        return false;
    }
    dev.busid.trim().eq_ignore_ascii_case(value)
}

/// An enabled rule with a usable value that matches `dev`.
pub fn rule_matches(rule: &FilterRule, dev: &UsbDevice) -> bool {
    if !rule.enabled {
        return false;
    }
    match rule.field {
        FilterField::DescriptionContains => description_contains(dev, &rule.value),
        FilterField::VidPid => vidpid_exact(dev, &rule.value),
        FilterField::BusId => busid_exact(dev, &rule.value),
    }
}

pub fn is_hidden(dev: &UsbDevice, rules: &[FilterRule]) -> bool {
    rules.iter().any(|r| rule_matches(r, dev))
}

pub fn hidden_count(devices: &[UsbDevice], rules: &[FilterRule]) -> usize {
    devices.iter().filter(|d| is_hidden(d, rules)).count()
}

/// Visible slice of the already-sorted discovery list. `show_all` is a session
/// toggle and must not be persisted. Order is preserved (numeric BUSID desc).
pub fn visible_devices<'a>(
    devices: &'a [UsbDevice],
    rules: &[FilterRule],
    show_all: bool,
) -> Vec<&'a UsbDevice> {
    if show_all {
        devices.iter().collect()
    } else {
        devices.iter().filter(|d| !is_hidden(d, rules)).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleError {
    EmptyValue,
    InvalidVidPid,
}

impl RuleError {
    pub fn i18n_key(self) -> &'static str {
        match self {
            Self::EmptyValue => "filter.empty_value",
            Self::InvalidVidPid => "filter.invalid_vidpid",
        }
    }
}

/// Validate a draft before save. Disabled rules may be incomplete.
pub fn validate_rules(rules: &[FilterRule]) -> Result<(), RuleError> {
    for rule in rules {
        if !rule.enabled {
            continue;
        }
        if rule.value.trim().is_empty() {
            return Err(RuleError::EmptyValue);
        }
        if rule.field == FilterField::VidPid && parse_vidpid(&rule.value).is_none() {
            return Err(RuleError::InvalidVidPid);
        }
    }
    Ok(())
}

fn rule_from_value(v: &Value) -> Option<FilterRule> {
    let obj = v.as_object()?;
    let field = match obj.get("field") {
        None => FilterField::DescriptionContains,
        Some(Value::String(s)) => match s.as_str() {
            "description_contains" | "" => FilterField::DescriptionContains,
            "vid_pid" => FilterField::VidPid,
            "bus_id" => FilterField::BusId,
            _ => return None,
        },
        _ => return None,
    };
    let value = match obj.get("value") {
        None => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.as_str()?.to_string(),
    };
    let enabled = match obj.get("enabled") {
        None => true,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(1) != 0,
        Some(Value::String(s)) => {
            let s = s.trim().to_ascii_lowercase();
            s != "false" && s != "0" && !s.is_empty()
        }
        _ => true,
    };
    Some(FilterRule { enabled, field, value })
}

pub fn load_rules(cfg: &Config) -> Vec<FilterRule> {
    match cfg.get_json(CONFIG_KEY) {
        Some(Value::Array(arr)) => arr.iter().filter_map(rule_from_value).collect(),
        _ => Vec::new(),
    }
}

pub fn store_rules(cfg: &mut Config, rules: &[FilterRule]) {
    let value = serde_json::to_value(rules).unwrap_or_else(|_| Value::Array(Vec::new()));
    cfg.set_json(CONFIG_KEY, value);
}
