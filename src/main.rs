//! UsbipdTool（Rust 版）入口：检测 usbipd → 主窗口。
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod gui;

use eframe::egui;

fn load_icon() -> Option<egui::IconData> {
    let png = include_bytes!(concat!(env!("OUT_DIR"), "/icon.png"));
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width: w, height: h })
}

fn main() -> eframe::Result {
    // 清单（requireAdministrator）失效时的兜底：自行以管理员身份重新启动
    let args: Vec<String> = std::env::args().collect();
    if cfg!(windows)
        && !usbipd_tool::core::usbipd::is_admin()
        && !args.iter().any(|a| a == "--elevated" || a == "--no-elevate")
        && gui::platform::relaunch_as_admin()
    {
        return Ok(());
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("UsbipdTool")
        .with_inner_size(gui::window_layout::DEFAULT_CLIENT_SIZE)
        .with_min_inner_size(gui::window_layout::MIN_CLIENT_SIZE);
    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "UsbipdTool",
        options,
        Box::new(|cc| {
            // ViewportBuilder sizes are logical points; the reference window is physical pixels.
            // Apply scale-adjusted inner and min sizes together so a 1.0-scale min width
            // cannot block the 150% default, and so the user cannot shrink narrower than default.
            let scale = cc.egui_ctx.native_pixels_per_point().unwrap_or(1.0);
            let frame = cc.egui_ctx.input(|i| {
                let viewport = i.viewport();
                match (viewport.outer_rect, viewport.inner_rect) {
                    (Some(outer), Some(inner)) if outer.width() >= inner.width() && outer.height() > inner.height() => outer.size() - inner.size(),
                    _ => egui::vec2(16.0, 39.0), // standard Windows decorated-frame fallback, in points
                }
            });
            let inner = gui::window_layout::initial_client_size(scale, frame);
            let min = gui::window_layout::min_client_size(scale, frame);
            cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(min));
            cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(inner));
            Ok(Box::new(gui::app::UsbipdApp::new(cc)))
        }),
    )
}
