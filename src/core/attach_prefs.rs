//! 附加偏好：设置页草稿、持久化与一键附加参数。

use super::actions::attach_args;
use super::config::Config;
use super::models::WslDistro;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachPrefs {
    pub use_default_distro: bool,
    pub distro: String,
    pub auto_attach: bool,
    pub host_ip: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefsError {
    DistroRequired,
    InvalidHostIp,
}

impl PrefsError {
    pub fn i18n_key(self) -> &'static str {
        match self {
            Self::DistroRequired => "settings.distro_required",
            Self::InvalidHostIp => "settings.invalid_host_ip",
        }
    }
}

impl AttachPrefs {
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            use_default_distro: cfg.use_default_wsl_distro(),
            distro: cfg.last_wsl_distro(),
            auto_attach: cfg.auto_attach_default(),
            host_ip: cfg.last_host_ip(),
        }
    }

    /// 保存草稿。勾选「使用默认发行版」时仍保留已记住的发行版名称，不清空。
    pub fn apply_to_config(&self, cfg: &mut Config) {
        cfg.set_bool("use_default_wsl_distro", self.use_default_distro);
        cfg.set_str("last_wsl_distro", self.distro.trim());
        cfg.set_str("last_host_ip", self.host_ip.trim());
        cfg.set_bool("auto_attach_default", self.auto_attach);
    }

    pub fn distribution(&self) -> Option<&str> {
        if self.use_default_distro {
            None
        } else {
            Some(self.distro.trim()).filter(|s| !s.is_empty())
        }
    }

    pub fn host_ip_value(&self) -> Option<&str> {
        Some(self.host_ip.trim()).filter(|s| !s.is_empty())
    }

    pub fn validate(&self) -> Result<(), PrefsError> {
        if !self.use_default_distro && self.distro.trim().is_empty() {
            return Err(PrefsError::DistroRequired);
        }
        if let Some(ip) = self.host_ip_value() {
            if !is_valid_host_ip(ip) {
                return Err(PrefsError::InvalidHostIp);
            }
        }
        Ok(())
    }

    pub fn attach_args(&self, busid: &str) -> Result<Vec<String>, PrefsError> {
        self.validate()?;
        Ok(attach_args(busid, self.distribution(), self.host_ip_value(), self.auto_attach))
    }

    /// 设置页命令预览：使用占位 busid，避免出现具体设备信息。
    pub fn preview_args(&self) -> Vec<String> {
        attach_args("<busid>", self.distribution(), self.host_ip_value(), self.auto_attach)
    }
}

pub fn is_valid_host_ip(s: &str) -> bool {
    s.parse::<std::net::IpAddr>().is_ok()
}

/// Detected default, or the first installed distro if the verbose listing has no `*` marker.
pub fn default_distro_name(distros: &[WslDistro]) -> Option<&str> {
    distros
        .iter()
        .find(|d| d.is_default)
        .or_else(|| distros.first())
        .map(|d| d.name.as_str())
}

/// If the draft has no usable explicit name (empty or not in the installed list),
/// return the detected default to fill the draft. Does not write config.
/// A name that matches an installed distro is preserved. Empty `distros` yields None.
pub fn fill_blank_or_invalid_draft(current: &str, distros: &[WslDistro]) -> Option<String> {
    if distros.is_empty() {
        return None;
    }
    let cur = current.trim();
    if !cur.is_empty() && distros.iter().any(|d| d.name == cur) {
        return None;
    }
    default_distro_name(distros).map(str::to_string)
}
