//! Headless visual regression checks; no hardware or native windows.
use egui;
mod platform { pub fn system_dark() -> Option<bool> { None } }
#[path = "../src/gui/theme.rs"]
mod theme;
use usbipd_tool::core::models::DeviceState;

#[test]
fn iot_palette_and_native_themes_are_preserved() {
    let ctx=egui::Context::default();
    for dark in [false,true] {
        theme::apply(&ctx,dark);
        assert_eq!(ctx.style_of(ctx.theme()).visuals.dark_mode,dark);
        assert_ne!(theme::state_color(DeviceState::Shared,dark),theme::state_color(DeviceState::NotShared,dark));
        assert_ne!(theme::log_bg(dark),theme::log_fg(dark));
        for kind in [theme::BtnKind::Bind,theme::BtnKind::Attach,theme::BtnKind::Detach,theme::BtnKind::Unbind] {
            assert_ne!(theme::button_color(kind,dark),theme::log_bg(dark));
        }
        let _=theme::preview_color(dark);
        let _=theme::border(dark);
    }
    assert_eq!(theme::state_color(DeviceState::Attached,true),egui::Color32::from_rgb(173,216,230));
    assert!(!theme::effective_dark("light",&ctx));
    assert!(theme::effective_dark("dark",&ctx));
}
