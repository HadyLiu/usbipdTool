//! UsbipdTool（Rust 版）入口：检测 usbipd → 主窗口。
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod gui;

use eframe::egui;

fn load_icon() -> Option<egui::IconData> {
    let ico = include_bytes!("../resources/icon.ico");
    let png = usbipd_tool::core::icon::ico_png_entry(ico, 64)?;
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
        .with_inner_size([880.0, 560.0])
        .with_min_inner_size([640.0, 400.0]);
    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "UsbipdTool",
        options,
        Box::new(|cc| Ok(Box::new(gui::app::UsbipdApp::new(cc)))),
    )
}
