//! IOT_Power 视觉语言：原生 egui 明暗底色、紧凑分组，以及仪表状态语义色。
//! 参考 HadyLiu/IOT_Power@4388a969 的 app/src/theme.rs 与 ui/status_panel.rs。
use super::egui;
use egui::Color32;
use usbipd_tool::core::models::DeviceState;

fn hex(s: &str) -> Color32 {
    Color32::from_hex(s).unwrap_or(Color32::GRAY)
}

pub fn effective_dark(pref: &str, ctx: &egui::Context) -> bool {
    match pref {
        "dark" => true,
        "light" => false,
        _ => super::platform::system_dark()
            .unwrap_or_else(|| ctx.system_theme() == Some(egui::Theme::Dark)),
    }
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
    // 与 IOT_Power 一致，使用 egui 原生底色，不再套用 Qt/QSS 调色板。
    ctx.set_theme(theme);
    ctx.set_visuals_of(theme, if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
    ctx.style_mut_of(theme, |style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.interact_size.y = 26.0;
    });
}

pub fn state_color(state: DeviceState, dark: bool) -> Color32 {
    hex(match (state, dark) {
        (DeviceState::NotShared, true) => "#f0e68c", // khaki / off
        (DeviceState::NotShared, false) => "#b8860b",
        (DeviceState::Shared, true) => "#90ee90", // lightgreen / on
        (DeviceState::Shared, false) => "#006400",
        (DeviceState::Attached, true) => "#add8e6", // lightblue
        (DeviceState::Attached, false) => "#00008b",
    })
}

#[derive(Clone, Copy)]
pub enum BtnKind { Bind, Attach, Detach, Unbind }

pub fn button_color(kind: BtnKind, dark: bool) -> Color32 {
    hex(match (kind, dark) {
        (BtnKind::Bind, true) => "#66cdaa", // mediumaquamarine
        (BtnKind::Bind, false) => "#228b22",
        (BtnKind::Attach, true) => "#add8e6",
        (BtnKind::Attach, false) => "#00008b",
        (BtnKind::Detach, true) => "#f0e68c",
        (BtnKind::Detach, false) => "#b8860b",
        (BtnKind::Unbind, true) => "#ff4500", // orangered
        (BtnKind::Unbind, false) => "#b22222",
    })
}

fn visuals(dark: bool) -> egui::Visuals {
    if dark { egui::Visuals::dark() } else { egui::Visuals::light() }
}
pub fn preview_color(dark: bool) -> Color32 { visuals(dark).weak_text_color() }
pub fn log_bg(dark: bool) -> Color32 { visuals(dark).extreme_bg_color }
pub fn log_fg(dark: bool) -> Color32 { visuals(dark).text_color() }
pub fn border(dark: bool) -> Color32 { visuals(dark).widgets.noninteractive.bg_stroke.color }
