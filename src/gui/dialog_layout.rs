//! 对话框共用布局：只限制宽度，按钮行只占一行高度。
use super::egui;

/// Preferred settings form width. Shrunk when the window is tighter.
pub const SETTINGS_FORM_WIDTH: f32 = 420.0;
/// Preferred filter-rule form width. Field, value, and delete stay on one row.
pub const FILTERS_FORM_WIDTH: f32 = 460.0;
/// Side inset kept between the form and the window edge.
pub const SETTINGS_VIEWPORT_MARGIN: f32 = 24.0;
/// Horizontal inset of a central panel (8 px each side).
/// Used by the headless layout tests to relate window size to the form column.
#[allow(dead_code)]
pub const WINDOW_PANEL_MARGIN: f32 = 16.0;
/// Scroll body cap. The area shrinks to its content, and the window hugs that height.
pub const SETTINGS_BODY_MAX: f32 = 400.0;
/// About six filter rows, then the list scrolls instead of growing without limit.
pub const FILTERS_BODY_MAX: f32 = 220.0;
/// Space kept under the last button when the window hugs its content.
pub const CONTENT_BOTTOM_PAD: f32 = 12.0;
/// Client size of the standalone settings window, in points.
/// Height matches the form (heading, fields, preview, buttons) plus a short pad.
/// The extra side margin covers egui grid/button overflow past the form column.
pub const SETTINGS_WINDOW_INNER: [f32; 2] = [
    SETTINGS_FORM_WIDTH + SETTINGS_VIEWPORT_MARGIN * 2.0,
    240.0,
];
/// The settings window cannot shrink below this client size.
pub const SETTINGS_WINDOW_MIN: [f32; 2] = [SETTINGS_WINDOW_INNER[0], SETTINGS_WINDOW_INNER[1]];
/// Client size of the standalone filter-rules window, in points.
/// Height matches the header, one rule row, and the buttons. More rules grow it, up to the body cap.
pub const FILTERS_WINDOW_INNER: [f32; 2] = [
    FILTERS_FORM_WIDTH + SETTINGS_VIEWPORT_MARGIN * 2.0,
    168.0,
];
/// The filter-rules window cannot shrink below this client size.
pub const FILTERS_WINDOW_MIN: [f32; 2] = [FILTERS_WINDOW_INNER[0], FILTERS_WINDOW_INNER[1]];

/// Form width that stays inside `viewport_width`, capped at `preferred`.
/// The headless layout tests cover this. Tool windows use [`column_width`].
#[allow(dead_code)]
pub fn fitted_width(viewport_width: f32, preferred: f32) -> f32 {
    let inner = (viewport_width - SETTINGS_VIEWPORT_MARGIN * 2.0).max(0.0);
    if inner <= 0.0 { preferred } else { preferred.min(inner) }
}

/// Settings form width that stays inside `viewport_width`.
#[allow(dead_code)]
pub fn fitted_form_width(viewport_width: f32) -> f32 {
    fitted_width(viewport_width, SETTINGS_FORM_WIDTH)
}

/// Column width inside a panel: use the available width, and do not grow past `preferred`.
pub fn column_width(available_width: f32, preferred: f32) -> f32 {
    if !available_width.is_finite() || available_width <= 1.0 {
        preferred
    } else {
        available_width.min(preferred)
    }
}

/// A full-size panel cannot be narrowed with `set_max_width` (its minimum is
/// already the panel). Allocate an explicit column so the form and buttons share one width.
pub fn show_column<R>(ui: &mut egui::Ui, width: f32, contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let height = ui.available_height();
    let h = if height.is_finite() { height.max(0.0) } else { 360.0 };
    ui.allocate_ui_with_layout(
        egui::vec2(width.max(0.0), h),
        egui::Layout::top_down(egui::Align::Min),
        contents,
    )
    .inner
}

pub fn apply_form_width(ui: &mut egui::Ui, width: f32) {
    let w = width.max(0.0);
    ui.set_min_width(w);
    ui.set_max_width(w);
    ui.set_width(w);
}

pub fn message(ui: &mut egui::Ui, text: &str) {
    ui.set_width(360.0);
    ui.strong("UsbipdTool");
    ui.add_space(6.0);
    ui.add(egui::Label::new(text).wrap());
    ui.add_space(10.0);
}

pub fn buttons<R>(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    // 单独调用 with_layout 会占用所有剩余高度。horizontal 先将高度约束为一行，
    // 再在这一行内右对齐，避免 Modal 被撑成大块空白。
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), contents).inner
    }).inner
}
