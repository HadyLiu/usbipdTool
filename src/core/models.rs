//! 数据模型。

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceState {
    NotShared,
    Shared,
    Attached,
}

impl DeviceState {
    /// usbipd list 文本中使用的英文状态名。
    pub fn from_list_text(s: &str) -> Option<Self> {
        match s {
            "Not shared" => Some(Self::NotShared),
            "Shared" => Some(Self::Shared),
            "Attached" => Some(Self::Attached),
            _ => None,
        }
    }

    /// 对应的 i18n key。
    pub fn i18n_key(self) -> &'static str {
        match self {
            Self::NotShared => "state.not_shared",
            Self::Shared => "state.shared",
            Self::Attached => "state.attached",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbDevice {
    pub busid: String,
    pub vid: String,
    pub pid: String,
    pub description: String,
    pub state: DeviceState,
    pub client_ip: Option<String>,
    pub instance_id: Option<String>,
    pub persisted_guid: Option<String>,
}

impl UsbDevice {
    pub fn vidpid(&self) -> String {
        if self.vid.is_empty() && self.pid.is_empty() {
            String::new()
        } else {
            format!("{}:{}", self.vid, self.pid)
        }
    }
    pub fn is_not_shared(&self) -> bool {
        self.state == DeviceState::NotShared
    }
    pub fn is_shared(&self) -> bool {
        self.state == DeviceState::Shared
    }
    pub fn is_attached(&self) -> bool {
        self.state == DeviceState::Attached
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WslDistro {
    pub name: String,
    pub is_default: bool,
}
