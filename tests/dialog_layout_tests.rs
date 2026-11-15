//! 无窗口布局回归测试，不执行 USB 命令或打开原生窗口。
use egui::{self, RichText};
use usbipd_tool::core::i18n::I18n;
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


#[test]
fn settings_buttons_align_with_form_width() {
    const FORM: f32 = 420.0;
    let [win_w, win_h] = dialog_layout::SETTINGS_WINDOW_INNER;
    let ctx = egui::Context::default();
    let mut form = egui::Rect::NOTHING;
    let mut buttons = egui::Rect::NOTHING;
    let mut confirm = egui::Rect::NOTHING;
    let mut column = egui::Rect::NOTHING;
    for _ in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(win_w, win_h))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let width = dialog_layout::column_width(ui.available_width(), FORM);
                dialog_layout::show_column(ui, width, |ui| {
                    dialog_layout::apply_form_width(ui, width);
                    ui.heading("设置");
                    form = egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_max_width(width);
                        ui.set_width(ui.available_width().min(width));
                        ui.label("WSL 2 目标");
                        ui.horizontal(|ui| {
                            ui.label("发行版");
                            let mut s = "Ubuntu".to_string();
                            ui.add(egui::TextEdit::singleline(&mut s).desired_width(220.0));
                        });
                    }).response.rect;
                    dialog_layout::buttons(ui, |ui| {
                        confirm = ui.button("确认").rect;
                        let cancel = ui.button("取消").rect;
                        buttons = confirm.union(cancel);
                    });
                    column = ui.min_rect();
                });
            });
        });
        output.textures_delta.clear();
    }
    let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(win_w, win_h));
    assert_inside("column", column, window);
    assert!(column.width() < 480.0, "settings column too wide {column:?}");
    assert!((form.width() - FORM).abs() < 16.0, "form {form:?}");
    // Button group right edge lines up with the form, not the leftover window width.
    assert!((buttons.right() - form.right()).abs() < 8.0, "buttons {buttons:?} form {form:?} column {column:?}");
    assert!(confirm.right() <= form.right() + 8.0, "confirm {confirm:?} form {form:?}");
    assert!((column.right() - form.right()).abs() < 24.0, "column much wider than form column={column:?} form={form:?}");
    assert!(form.right() < win_w, "form reached past the window edge {form:?}");
}


fn flatten_shapes(shape: &egui::Shape, out: &mut Vec<egui::Shape>) {
    match shape {
        egui::Shape::Vec(children) => {
            for child in children {
                flatten_shapes(child, out);
            }
        }
        other => out.push(other.clone()),
    }
}

fn all_flat_shapes(output: &egui::FullOutput) -> Vec<egui::Shape> {
    let mut flat = Vec::new();
    for clipped in &output.shapes {
        flatten_shapes(&clipped.shape, &mut flat);
    }
    flat
}

fn painted_text_rects(output: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    let mut texts = Vec::new();
    for shape in all_flat_shapes(output) {
        if let egui::Shape::Text(ts) = shape {
            let visual = ts.galley.mesh_bounds.translate(ts.pos.to_vec2());
            texts.push((ts.galley.job.text.clone(), visual));
        }
    }
    texts
}

fn text_matching<'a>(painted: &'a [(String, egui::Rect)], needle: &str) -> Option<&'a (String, egui::Rect)> {
    painted
        .iter()
        .filter(|(_, r)| r.width() > 0.5 && r.height() > 0.5)
        .find(|(s, _)| s == needle || s.contains(needle))
}

struct PaintedSettings {
    /// Compact form column inside the standalone window.
    column: egui::Rect,
    heading: egui::Rect,
    confirm: egui::Rect,
    cancel: egui::Rect,
    form_w: f32,
    painted: Vec<(String, egui::Rect)>,
}

fn production_auto_attach_is_short(tr: &I18n) {
    let label = tr.tr("attach_dialog.auto_attach");
    assert!(
        !label.contains("usbipd")
            && !label.contains("看管")
            && !label.contains("不会自动再附加")
            && !label.contains("will not be auto-restored")
            && !label.contains("while the app is running"),
        "settings UI must not include the long auto-reattach explanation, got {label:?}"
    );
    let chars = label.chars().count();
    assert!(
        chars <= 36,
        "settings auto-reattach checkbox should stay a short label, got {chars} chars: {label:?}"
    );
}

/// Mirror of `SettingsDialog::show` inside its own window client area.
fn paint_settings_window(lang: &str, width: f32, height: f32) -> PaintedSettings {
    let tr = I18n::new(lang);
    production_auto_attach_is_short(&tr);

    let ctx = egui::Context::default();
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut last = None;
    let mut last_output = egui::FullOutput::default();
    for frame in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(viewport),
            ..Default::default()
        };
        last_output = ctx.run_ui(input, |ui| {
            let mut auto = true;
            let mut use_def = true;
            let mut distro = "Ubuntu".to_string();
            let mut host = String::new();
            let mut heading = egui::Rect::NOTHING;
            let mut confirm = egui::Rect::NOTHING;
            let mut cancel = egui::Rect::NOTHING;
            let mut column = egui::Rect::NOTHING;
            let mut form_w = dialog_layout::SETTINGS_FORM_WIDTH;
            egui::CentralPanel::default().show(ui, |ui| {
                form_w = dialog_layout::column_width(ui.available_width(), dialog_layout::SETTINGS_FORM_WIDTH);
                dialog_layout::show_column(ui, form_w, |ui| {
                dialog_layout::apply_form_width(ui, form_w);
                heading = ui.heading(tr.tr("settings.title")).rect;
                ui.add_space(6.0);
                let max_body = dialog_layout::SETTINGS_BODY_MAX;
                egui::ScrollArea::vertical()
                    .id_salt("settings_body_scroll")
                    .max_height(max_body)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        dialog_layout::apply_form_width(ui, form_w);
                        let field_w = (form_w - 120.0).clamp(80.0, form_w);
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.set_max_width(form_w);
                            ui.set_width(ui.available_width().min(form_w));
                            ui.strong(tr.tr("attach_dialog.group_wsl"));
                            egui::Grid::new("settings_form")
                                .num_columns(2)
                                .spacing([10.0, 8.0])
                                .max_col_width(form_w)
                                .show(ui, |ui| {
                                    ui.label("");
                                    let use_default = tr.trf(
                                        "attach_dialog.use_default_named",
                                        &[("name", "Ubuntu")],
                                    );
                                    ui.checkbox(&mut use_def, use_default);
                                    ui.end_row();

                                    ui.label(tr.tr("attach_dialog.distro"));
                                    ui.add(egui::TextEdit::singleline(&mut distro).desired_width(field_w));
                                    ui.end_row();

                                    ui.label("");
                                    ui.checkbox(&mut auto, tr.tr("attach_dialog.auto_attach"));
                                    ui.end_row();

                                    ui.label(tr.tr("attach_dialog.host_ip"));
                                    ui.add(
                                        egui::TextEdit::singleline(&mut host)
                                            .hint_text(tr.tr("attach_dialog.host_ip_placeholder"))
                                            .desired_width(field_w),
                                    );
                                    ui.end_row();
                                });
                        });
                        let preview = tr.trf(
                            "attach_dialog.preview",
                            &[("cmd", "usbipd attach --wsl Ubuntu --busid 1-1")],
                        );
                        ui.add(egui::Label::new(preview).wrap());
                    });
                ui.add_space(8.0);
                dialog_layout::buttons(ui, |ui| {
                    confirm = ui.button(RichText::new(tr.tr("common.confirm")).strong()).rect;
                    cancel = ui.button(tr.tr("common.cancel")).rect;
                });
                column = ui.min_rect();
                });
            });
            last = Some(PaintedSettings {
                column,
                heading,
                confirm,
                cancel,
                form_w,
                painted: Vec::new(),
            });
            let _ = frame;
        });
        last_output.textures_delta.clear();
    }
    let mut painted = last.expect("settings window painted");
    painted.painted = painted_text_rects(&last_output);
    painted
}

fn assert_inside(label: &str, inner: egui::Rect, outer: egui::Rect) {
    const EPS: f32 = 1.0;
    assert!(
        inner.left() >= outer.left() - EPS
            && inner.right() <= outer.right() + EPS
            && inner.top() >= outer.top() - EPS
            && inner.bottom() <= outer.bottom() + EPS
            && inner.width() > 1.0
            && inner.height() > 1.0,
        "{label} {inner:?} is not inside {outer:?}"
    );
}

#[test]
fn fitted_form_width_caps_to_viewport_and_keeps_preferred_when_room() {
    assert!((dialog_layout::fitted_form_width(1024.0) - dialog_layout::SETTINGS_FORM_WIDTH).abs() < 1e-3);
    assert!((dialog_layout::fitted_form_width(1008.0) - dialog_layout::SETTINGS_FORM_WIDTH).abs() < 1e-3);
    assert!((dialog_layout::fitted_form_width(670.0) - dialog_layout::SETTINGS_FORM_WIDTH).abs() < 1e-3);
    assert!((dialog_layout::fitted_form_width(666.667) - dialog_layout::SETTINGS_FORM_WIDTH).abs() < 1e-3);
    assert!((dialog_layout::fitted_form_width(640.0) - dialog_layout::SETTINGS_FORM_WIDTH).abs() < 1e-3);
    let tight = 400.0;
    let expected = (tight - dialog_layout::SETTINGS_VIEWPORT_MARGIN * 2.0).max(0.0);
    assert!((dialog_layout::fitted_form_width(tight) - expected).abs() < 1e-3);
}

#[test]
fn settings_window_content_stays_inside_client_with_production_strings() {
    let [dw, dh] = dialog_layout::SETTINGS_WINDOW_INNER;
    let [mw, mh] = dialog_layout::SETTINGS_WINDOW_MIN;
    // Default client, the minimum the user can resize to, and a larger window
    // that must not stretch the form out to the edges.
    let viewports = [
        (dw, dh),
        (mw, mh),
        (720.0, 560.0),
    ];
    for lang in ["zh_CN", "en_US"] {
        let tr = I18n::new(lang);
        let title = tr.tr("settings.title");
        let confirm_s = tr.tr("common.confirm");
        let auto = tr.tr("attach_dialog.auto_attach");
        production_auto_attach_is_short(&tr);
        for (w, h) in viewports {
            let painted = paint_settings_window(lang, w, h);
            let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));
            let label = format!("{lang} {w}x{h}");
            assert_inside(&format!("{label} column"), painted.column, viewport);
            assert_inside(&format!("{label} heading widget"), painted.heading, viewport);
            assert_inside(&format!("{label} heading in column"), painted.heading, painted.column);
            assert_inside(&format!("{label} confirm"), painted.confirm, viewport);
            assert_inside(&format!("{label} confirm in column"), painted.confirm, painted.column);
            assert_inside(&format!("{label} cancel"), painted.cancel, viewport);
            assert_inside(&format!("{label} cancel in column"), painted.cancel, painted.column);

            assert!(
                painted.column.width() <= painted.form_w + 48.0,
                "{label} column {:?} wider than fitted form {}",
                painted.column,
                painted.form_w
            );
            assert!(
                painted.heading.right() <= w && painted.confirm.right() <= w,
                "{label} heading {:?} or confirm {:?} cropped at right edge w={w}",
                painted.heading,
                painted.confirm
            );
            if w > dialog_layout::SETTINGS_WINDOW_INNER[0] + 40.0 {
                assert!(
                    painted.column.right() < w - 40.0,
                    "{label} column {:?} stretched to the window edge",
                    painted.column
                );
            }

            let title_g = text_matching(&painted.painted, &title)
                .unwrap_or_else(|| panic!("{label} missing painted title {title:?} in {:?}", painted.painted));
            assert_inside(&format!("{label} painted title"), title_g.1, viewport);
            assert_inside(&format!("{label} painted title in column"), title_g.1, painted.column);

            let confirm_g = text_matching(&painted.painted, &confirm_s)
                .unwrap_or_else(|| panic!("{label} missing painted confirm {confirm_s:?} in {:?}", painted.painted));
            assert_inside(&format!("{label} painted confirm"), confirm_g.1, viewport);
            assert_inside(&format!("{label} painted confirm in column"), confirm_g.1, painted.column);

            let auto_g = text_matching(&painted.painted, &auto)
                .unwrap_or_else(|| panic!("{label} missing painted auto-reattach {auto:?} in {:?}", painted.painted));
            assert_inside(&format!("{label} painted auto-reattach"), auto_g.1, viewport);
            assert_inside(&format!("{label} painted auto-reattach in column"), auto_g.1, painted.column);
        }
    }
}


/// Mirror of the filter-rules window chrome with production zh/en strings.
fn paint_filters_window(lang: &str, width: f32, height: f32) -> PaintedSettings {
    let tr = I18n::new(lang);
    let ctx = egui::Context::default();
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut last = None;
    let mut last_output = egui::FullOutput::default();
    for _ in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(viewport),
            ..Default::default()
        };
        last_output = ctx.run_ui(input, |ui| {
            let mut enabled = true;
            let mut value = "camera".to_string();
            let mut heading = egui::Rect::NOTHING;
            let mut confirm = egui::Rect::NOTHING;
            let mut cancel = egui::Rect::NOTHING;
            let mut column = egui::Rect::NOTHING;
            let mut form_w = dialog_layout::FILTERS_FORM_WIDTH;
            egui::CentralPanel::default().show(ui, |ui| {
                form_w = dialog_layout::column_width(ui.available_width(), dialog_layout::FILTERS_FORM_WIDTH);
                dialog_layout::show_column(ui, form_w, |ui| {
                dialog_layout::apply_form_width(ui, form_w);
                heading = ui.heading(tr.tr("filter.title")).rect;
                ui.add_space(4.0);
                ui.add(egui::Label::new(RichText::new(tr.tr("filter.semantics")).small()).wrap());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(tr.trf("filter.hidden_count", &[("n", "2")]));
                    let _ = ui.button(tr.tr("filter.show_all"));
                });
                let max_body = dialog_layout::FILTERS_BODY_MAX;
                egui::ScrollArea::vertical()
                    .id_salt("filter_rules_scroll")
                    .max_height(max_body)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        dialog_layout::apply_form_width(ui, form_w);
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut enabled, "");
                            let _ = ui.button(tr.tr("filter.field.description"));
                            ui.add(egui::TextEdit::singleline(&mut value).desired_width((form_w - 200.0).clamp(80.0, form_w)));
                            let _ = ui.button(tr.tr("filter.delete"));
                        });
                        let _ = ui.button(tr.tr("filter.add"));
                    });
                ui.add_space(8.0);
                dialog_layout::buttons(ui, |ui| {
                    confirm = ui.button(RichText::new(tr.tr("common.save")).strong()).rect;
                    cancel = ui.button(tr.tr("common.cancel")).rect;
                });
                column = ui.min_rect();
                });
            });
            last = Some(PaintedSettings {
                column,
                heading,
                confirm,
                cancel,
                form_w,
                painted: Vec::new(),
            });
        });
        last_output.textures_delta.clear();
    }
    let mut painted = last.expect("filters window painted");
    painted.painted = painted_text_rects(&last_output);
    painted
}

#[test]
fn filters_window_content_stays_inside_client_with_production_strings() {
    let [dw, dh] = dialog_layout::FILTERS_WINDOW_INNER;
    let [mw, mh] = dialog_layout::FILTERS_WINDOW_MIN;
    let viewports = [
        (dw, dh),
        (mw, mh),
        (720.0, 560.0),
    ];
    for lang in ["zh_CN", "en_US"] {
        let tr = I18n::new(lang);
        let title = tr.tr("filter.title");
        let save = tr.tr("common.save");
        for (w, h) in viewports {
            let painted = paint_filters_window(lang, w, h);
            let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));
            let label = format!("{lang} {w}x{h}");
            assert_inside(&format!("{label} column"), painted.column, viewport);
            assert_inside(&format!("{label} heading"), painted.heading, viewport);
            assert_inside(&format!("{label} heading in column"), painted.heading, painted.column);
            assert_inside(&format!("{label} save"), painted.confirm, viewport);
            assert_inside(&format!("{label} save in column"), painted.confirm, painted.column);
            assert_inside(&format!("{label} cancel"), painted.cancel, viewport);
            assert!(
                painted.column.width() <= painted.form_w + 48.0,
                "{label} column {:?} wider than fitted form {}",
                painted.column,
                painted.form_w
            );
            assert!(
                painted.heading.right() <= w && painted.confirm.right() <= w,
                "{label} heading {:?} or save {:?} cropped w={w}",
                painted.heading,
                painted.confirm
            );
            let title_g = text_matching(&painted.painted, &title)
                .unwrap_or_else(|| panic!("{label} missing title {title:?} in {:?}", painted.painted));
            assert_inside(&format!("{label} painted title"), title_g.1, viewport);
            let save_g = text_matching(&painted.painted, &save)
                .unwrap_or_else(|| panic!("{label} missing save {save:?} in {:?}", painted.painted));
            assert_inside(&format!("{label} painted save"), save_g.1, viewport);
        }
    }
}

#[test]
fn tool_windows_hug_their_content() {
    let [sw, sh] = dialog_layout::SETTINGS_WINDOW_INNER;
    let [fw, fh] = dialog_layout::FILTERS_WINDOW_INNER;
    for lang in ["zh_CN", "en_US"] {
        let s = paint_settings_window(lang, sw, sh);
        let settings_gap = sh - s.column.bottom();
        let pad = dialog_layout::CONTENT_BOTTOM_PAD;
        assert!(
            settings_gap >= pad - 4.0 && settings_gap <= pad + 12.0,
            "{lang} settings window leaves {settings_gap} px under the buttons, column {:?}",
            s.column
        );
        let f = paint_filters_window(lang, fw, fh);
        let filters_gap = fh - f.column.bottom();
        assert!(
            filters_gap >= pad - 4.0 && filters_gap <= pad + 12.0,
            "{lang} filters window leaves {filters_gap} px under the buttons, column {:?}",
            f.column
        );
    }
}

#[test]
fn tool_windows_fit_their_preferred_forms() {
    let (sw, sh) = (dialog_layout::SETTINGS_WINDOW_INNER[0], dialog_layout::SETTINGS_WINDOW_INNER[1]);
    let settings_fit = dialog_layout::SETTINGS_FORM_WIDTH + dialog_layout::SETTINGS_VIEWPORT_MARGIN * 2.0;
    assert!((sw - settings_fit).abs() < 1e-3);
    assert!(sh + 0.1 >= dialog_layout::SETTINGS_WINDOW_MIN[1]);
    let settings_inner = sw - dialog_layout::WINDOW_PANEL_MARGIN;
    assert!((dialog_layout::column_width(settings_inner, dialog_layout::SETTINGS_FORM_WIDTH)
        - dialog_layout::SETTINGS_FORM_WIDTH).abs() < 1e-3);

    let fw = dialog_layout::FILTERS_WINDOW_INNER[0];
    let filters_fit = dialog_layout::FILTERS_FORM_WIDTH + dialog_layout::SETTINGS_VIEWPORT_MARGIN * 2.0;
    assert!((fw - filters_fit).abs() < 1e-3);
    let filters_inner = fw - dialog_layout::WINDOW_PANEL_MARGIN;
    assert!((dialog_layout::column_width(filters_inner, dialog_layout::FILTERS_FORM_WIDTH)
        - dialog_layout::FILTERS_FORM_WIDTH).abs() < 1e-3);
    assert!((dialog_layout::FILTERS_WINDOW_MIN[0] - fw).abs() < 1e-3);
    assert!((dialog_layout::SETTINGS_WINDOW_MIN[0] - sw).abs() < 1e-3);
}
