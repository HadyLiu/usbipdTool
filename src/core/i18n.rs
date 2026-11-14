//! 轻量 i18n：复用原版 JSON 词典（编译期嵌入）+ `tr()`。

use std::collections::HashMap;

pub const SUPPORTED: &[&str] = &["zh_CN", "en_US"];

const ZH_CN: &str = include_str!("../../resources/i18n/zh_CN.json");
const EN_US: &str = include_str!("../../resources/i18n/en_US.json");

#[derive(Debug, Clone)]
pub struct I18n {
    lang: String,
    dict: HashMap<String, String>,
}

fn parse_dict(text: &str) -> HashMap<String, String> {
    let v: serde_json::Value = serde_json::from_str(text).unwrap_or(serde_json::Value::Null);
    let mut m = HashMap::new();
    if let serde_json::Value::Object(o) = v {
        for (k, v) in o {
            let s = match v {
                serde_json::Value::String(s) => s,
                other => other.to_string(),
            };
            m.insert(k, s);
        }
    }
    m
}

/// 原始词典文本（测试用）。
pub fn raw_dict(lang: &str) -> &'static str {
    if lang == "en_US" { EN_US } else { ZH_CN }
}

impl I18n {
    /// 加载语言；不支持的语言回退到 zh_CN（与原版一致）。
    pub fn new(lang: &str) -> Self {
        let lang = if SUPPORTED.contains(&lang) { lang } else { "zh_CN" };
        Self { lang: lang.to_string(), dict: parse_dict(raw_dict(lang)) }
    }

    pub fn lang(&self) -> &str {
        &self.lang
    }

    pub fn tr(&self, key: &str) -> String {
        self.dict.get(key).cloned().unwrap_or_else(|| key.to_string())
    }

    /// 带 `{name}` 占位符替换的翻译。
    pub fn trf(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut s = self.tr(key);
        for (k, v) in args {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        s
    }
}

/// 根据区域名（如 `zh-CN`、`zh_CN.UTF-8`、`en-US`）判断语言。
pub fn lang_from_locale(name: &str) -> &'static str {
    if name.to_ascii_lowercase().starts_with("zh") { "zh_CN" } else { "en_US" }
}

/// 检测系统语言。
#[cfg(windows)]
pub fn detect_system_lang() -> &'static str {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
    let mut buf = [0u16; 85];
    let n = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
    if n > 0 {
        let name = String::from_utf16_lossy(&buf[..(n as usize).saturating_sub(1)]);
        return lang_from_locale(&name);
    }
    "en_US"
}

#[cfg(not(windows))]
pub fn detect_system_lang() -> &'static str {
    let name = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    lang_from_locale(&name)
}
