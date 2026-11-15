//! 配置读写（`%APPDATA%\UsbipdTool\config.json`），键名与原版一致。

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub const APP_NAME: &str = "UsbipdTool";

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// 原始 JSON（保留未知键，与原版 dict.update 行为一致）
    data: Map<String, Value>,
}

fn defaults() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("language".into(), Value::String(String::new())); // "" = 跟随系统
    m.insert("theme".into(), Value::String(String::new())); // "" = 跟随系统；light / dark
    m.insert("force_bind_default".into(), Value::Bool(false));
    m.insert("last_wsl_distro".into(), Value::String(String::new()));
    m.insert("last_host_ip".into(), Value::String(String::new()));
    m.insert("auto_attach_default".into(), Value::Bool(false));
    m.insert("use_default_wsl_distro".into(), Value::Bool(true));
    m
}

impl Default for Config {
    fn default() -> Self {
        Self { data: defaults() }
    }
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(APP_NAME)
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

impl Config {
    pub fn from_json_str(text: &str) -> Self {
        let mut data = defaults();
        if let Ok(Value::Object(loaded)) = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) {
            for (k, v) in loaded {
                data.insert(k, v);
            }
        }
        Self { data }
    }

    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(t) => Self::from_json_str(&t),
            Err(_) => Self::default(),
        }
    }

    pub fn load() -> Self {
        Self::load_from(&config_path())
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(&Value::Object(self.data.clone())).unwrap_or_else(|_| "{}".into())
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(path, self.to_json_string())
    }

    /// 保存失败时静默忽略（与原版一致）。
    pub fn save(&self) {
        let _ = self.save_to(&config_path());
    }

    pub fn get_str(&self, key: &str) -> String {
        match self.data.get(key) {
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        }
    }

    pub fn get_bool(&self, key: &str) -> bool {
        match self.data.get(key) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
            Some(Value::String(s)) => !s.is_empty(),
            _ => false,
        }
    }

    pub fn set_str(&mut self, key: &str, v: impl Into<String>) {
        self.data.insert(key.into(), Value::String(v.into()));
    }

    pub fn set_bool(&mut self, key: &str, v: bool) {
        self.data.insert(key.into(), Value::Bool(v));
    }

    pub fn get_json(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    pub fn set_json(&mut self, key: &str, v: Value) {
        self.data.insert(key.into(), v);
    }

    pub fn language(&self) -> String {
        self.get_str("language")
    }
    pub fn theme(&self) -> String {
        self.get_str("theme")
    }
    pub fn force_bind_default(&self) -> bool {
        self.get_bool("force_bind_default")
    }
    pub fn last_wsl_distro(&self) -> String {
        self.get_str("last_wsl_distro")
    }
    pub fn last_host_ip(&self) -> String {
        self.get_str("last_host_ip")
    }
    pub fn auto_attach_default(&self) -> bool {
        self.get_bool("auto_attach_default")
    }
    /// Missing key (older configs) means use the WSL default distro.
    pub fn use_default_wsl_distro(&self) -> bool {
        match self.data.get("use_default_wsl_distro") {
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_i64().unwrap_or(1) != 0,
            Some(Value::String(s)) => {
                let s = s.trim().to_ascii_lowercase();
                s != "false" && s != "0" && !s.is_empty()
            }
            None => true,
            _ => true,
        }
    }
}
