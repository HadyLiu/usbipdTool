//! 无窗口布局回归测试，不执行 USB 命令或打开原生窗口。
use egui;
#[path = "../src/gui/dialog_layout.rs"]
mod dialog_layout;

fn measure(text: &str, width: f32, height: f32, two_buttons: bool) -> egui::Rect {
    let ctx = egui::Context::default();
    let mut rect = egui::Rect::NOTHING;
    // 多帧覆盖 Area 首帧 sizing pass 与后续缓存布局。
    for _ in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |_ui| {
            rect = egui::Modal::new(egui::Id::unique("test_modal")).show(&ctx, |ui| {
                dialog_layout::message(ui, text);
                dialog_layout::buttons(ui, |ui| {
                    let _ = ui.button("No");
                    if two_buttons { let _ = ui.button("Yes"); }
                });
            }).response.rect;
        });
        output.textures_delta.clear();
    }
    rect
}

#[test]
fn confirmation_modal_stays_compact_on_large_viewport() {
    for (w, h) in [(880.0, 560.0), (1920.0, 1080.0)] {
        for text in ["Detach device 10-1?", "确定要分离设备 10-1 吗？"] {
            let r = measure(text, w, h, true);
            assert!(r.width() < 410.0, "{r:?}");
            assert!(r.height() < 160.0, "{r:?}");
        }
    }
}

#[test]
fn warning_message_wraps_without_viewport_sized_blank_space() {
    let text = "usbipd was not found. Please install usbipd-win first. Run in an elevated PowerShell: winget install usbipd-win. Then reopen this application. ".repeat(3);
    let r = measure(&text, 1920.0, 1080.0, false);
    assert!(r.width() < 410.0, "{r:?}");
    assert!(r.height() > 100.0 && r.height() < 440.0, "{r:?}");
}
