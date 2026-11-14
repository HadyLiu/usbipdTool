//! 对话框共用布局：只限制宽度，按钮行只占一行高度。
use super::egui;

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
