//! 主题（浅色/深色/跟随系统），配色与原版 QSS 调色板一致。

use eframe::egui::{self, Color32, Stroke};
use usbipd_tool::core::models::DeviceState;

fn hex(s: &str) -> Color32 {
    Color32::from_hex(s).unwrap_or(Color32::GRAY)
}

/// 根据偏好（"" / "light" / "dark"）得出是否使用深色。
pub fn effective_dark(pref: &str, ctx: &egui::Context) -> bool {
    match pref {
        "dark" => true,
        "light" => false,
        _ => super::platform::system_dark()
            .unwrap_or_else(|| ctx.system_theme() == Some(egui::Theme::Dark)),
    }
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let (window, text, base, alt, button, border, sel, header, hover) = if dark {
        ("#1b1c20", "#e8eaed", "#222329", "#282a30", "#2f3136", "#4a4d55", "#3d6ee0", "#2b2d33", "#3a3d45")
    } else {
        ("#f5f6f8", "#1f2328", "#ffffff", "#fafbfc", "#ffffff", "#c4c9d0", "#2f6fed", "#eef0f3", "#e8f0fe")
    };
    v.panel_fill = hex(window);
    v.window_fill = hex(window);
    v.extreme_bg_color = hex(base);
    v.faint_bg_color = hex(alt);
    v.override_text_color = Some(hex(text));
    v.selection.bg_fill = hex(sel);
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.code_bg_color = hex(header);
    let stroke = Stroke::new(1.0, hex(border));
    v.widgets.inactive.bg_fill = hex(button);
    v.widgets.inactive.weak_bg_fill = hex(button);
    v.widgets.inactive.bg_stroke = stroke;
    v.widgets.hovered.bg_fill = hex(hover);
    v.widgets.hovered.weak_bg_fill = hex(hover);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, hex(if dark { "#5a5e68" } else { "#7fb0e8" }));
    v.widgets.active.weak_bg_fill = hex(if dark { "#42454e" } else { "#d6e4f7" });
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, hex(if dark { "#3a3c44" } else { "#d9dce1" }));
    let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
    ctx.set_theme(theme);
    ctx.set_visuals_of(theme, v);
}

pub fn state_color(state: DeviceState, dark: bool) -> Color32 {
    hex(match (state, dark) {
        (DeviceState::NotShared, false) => "#6b7280",
        (DeviceState::Shared, false) => "#1a7f37",
        (DeviceState::Attached, false) => "#0969da",
        (DeviceState::NotShared, true) => "#9ca3af",
        (DeviceState::Shared, true) => "#3fb950",
        (DeviceState::Attached, true) => "#58a6ff",
    })
}

#[derive(Clone, Copy)]
pub enum BtnKind {
    Bind,
    Attach,
    Detach,
    Unbind,
}

pub fn button_color(kind: BtnKind, dark: bool) -> Color32 {
    hex(match (kind, dark) {
        (BtnKind::Bind, false) => "#1a7f37",
        (BtnKind::Attach, false) => "#0969da",
        (BtnKind::Detach, false) => "#b35900",
        (BtnKind::Unbind, false) => "#cf222e",
        (BtnKind::Bind, true) => "#3fb950",
        (BtnKind::Attach, true) => "#58a6ff",
        (BtnKind::Detach, true) => "#d29922",
        (BtnKind::Unbind, true) => "#f85149",
    })
}

pub fn preview_color(dark: bool) -> Color32 {
    hex(if dark { "#9aa0a8" } else { "#57606a" })
}

pub fn log_bg(dark: bool) -> Color32 {
    hex(if dark { "#141519" } else { "#f6f8fa" })
}

pub fn log_fg(dark: bool) -> Color32 {
    hex(if dark { "#d4d6db" } else { "#24292f" })
}
