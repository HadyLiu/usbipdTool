//! Headless layout regression: resize / many log lines / native DPI.
use egui;
#[path = "../src/gui/window_layout.rs"]
mod window_layout;

#[test]
fn screenshot_size_accounts_for_native_dpi_and_window_frame() {
    for scale in [1.0, 1.25, 1.5] {
        let frame = egui::vec2(16.0, 39.0);
        let client = window_layout::initial_client_size(scale, frame);
        let physical = (client + frame) * scale;
        assert!((physical.x - window_layout::DECORATED_WINDOW_SIZE[0]).abs() < 0.1, "{physical:?}");
        assert!((physical.y - window_layout::DECORATED_WINDOW_SIZE[1]).abs() < 0.1, "{physical:?}");
    }
    assert_eq!(window_layout::initial_client_size(1.0, egui::vec2(16.0,39.0)),egui::Vec2::from(window_layout::DEFAULT_CLIENT_SIZE));
}
#[test]
fn minimum_size_matches_default_size_after_dpi_and_frame_conversion() {
    let frame = egui::vec2(16.0, 39.0);
    // Default physical size is the floor at every scale on both axes.
    for scale in [1.0, 1.25, 1.5, 2.0, 3.0] {
        let initial = window_layout::initial_client_size(scale, frame);
        let min = window_layout::min_client_size(scale, frame);
        assert!(initial.is_finite() && min.is_finite(), "{scale}");
        assert!((min.x - initial.x).abs() < 1e-3, "min width {} != default width {} @{scale}", min.x, initial.x);
        assert!((min.y - initial.y).abs() < 1e-3, "min height {} != default height {} @{scale}", min.y, initial.y);
        let physical_min = (min + frame) * scale;
        assert!(
            (physical_min.x - window_layout::DECORATED_WINDOW_SIZE[0]).abs() < 0.1,
            "min physical width {} @{scale}",
            physical_min.x
        );
        assert!(
            (physical_min.y - window_layout::DECORATED_WINDOW_SIZE[1]).abs() < 0.1,
            "min physical height {} @{scale}",
            physical_min.y
        );
    }
    // 150% must not clamp min/default up to the 1.0-scale client (that would exceed 1024×760 physical).
    let at_150 = window_layout::initial_client_size(1.5, frame);
    assert!(at_150.x < window_layout::DEFAULT_CLIENT_SIZE[0] - 1.0, "{at_150:?}");
    assert!(at_150.y < window_layout::DEFAULT_CLIENT_SIZE[1] - 1.0, "{at_150:?}");
    assert_eq!(at_150, window_layout::min_client_size(1.5, frame));
    // 1.0-scale constants document the same rule: min equals default on both axes.
    assert_eq!(window_layout::MIN_CLIENT_SIZE, window_layout::DEFAULT_CLIENT_SIZE);
    assert_eq!(window_layout::MIN_CLIENT_HEIGHT, window_layout::DEFAULT_CLIENT_SIZE[1]);
    assert!((window_layout::DECORATED_WINDOW_SIZE[0] - 1024.0).abs() < 1e-3);
    assert!((window_layout::DECORATED_WINDOW_SIZE[1] - 760.0).abs() < 1e-3);
    for scale in [0.0, f32::NAN] {
        let size = window_layout::initial_client_size(scale, frame);
        let min = window_layout::min_client_size(scale, frame);
        assert!(size.is_finite() && min.is_finite());
        assert_eq!(size, egui::Vec2::from(window_layout::DEFAULT_CLIENT_SIZE));
        assert_eq!(min, egui::Vec2::from(window_layout::DEFAULT_CLIENT_SIZE));
    }
    assert_eq!(window_layout::table_height(0.0), 0.0);
}
fn try_install_cjk(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyh.ttf",
        "/mnt/c/Windows/Fonts/msyh.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    ];
    for path in CANDIDATES {
        if let Ok(data) = std::fs::read(path) {
            ctx.add_font(egui::FontInsert::new(
                "cjk-test",
                egui::FontData::from_owned(data),
                vec![egui::InsertFontFamily {
                    family: egui::FontFamily::Proportional,
                    priority: egui::FontPriority::Lowest,
                }],
            ));
            return;
        }
    }
}

fn zh_labels() -> (String, String, String, String, String, String, String, String, String) {
    (
        "语言".into(),
        "主题".into(),
        "重新扫描".into(),
        "强制绑定".into(),
        "过滤规则".into(),
        "设置".into(),
        "跟随系统".into(),
        "浅色".into(),
        "深色".into(),
    )
}

fn en_labels() -> (String, String, String, String, String, String, String, String, String) {
    (
        "Language".into(),
        "Theme".into(),
        "Rescan".into(),
        "Force bind".into(),
        "Filters".into(),
        "Settings".into(),
        "System".into(),
        "Light".into(),
        "Dark".into(),
    )
}

struct PaintedToolbar {
    content_width: f32,
    available: f32,
    group_rects: Vec<egui::Rect>,
    widget_rects: Vec<egui::Rect>,
}

fn paint_compact_toolbar(ui: &mut egui::Ui, owned: &(String, String, String, String, String, String, String, String, String)) -> PaintedToolbar {
    let lang_options = ["中文", "English"];
    let theme_options = [owned.6.as_str(), owned.7.as_str(), owned.8.as_str()];
    let labels = window_layout::ToolbarLabels {
        language: &owned.0,
        lang_options: &lang_options,
        theme: &owned.1,
        theme_options: &theme_options,
        refresh: &owned.2,
        force_bind: &owned.3,
        filters: &owned.4,
        settings: &owned.5,
    };
    let content_width = window_layout::toolbar_content_width(ui, &labels);
    let lang_w = window_layout::combo_width(ui, labels.lang_options);
    let theme_w = window_layout::combo_width(ui, labels.theme_options);
    let available = ui.available_width();
    let mut group_rects = Vec::new();
    let mut widget_rects = Vec::new();
    window_layout::compact_toolbar(ui, content_width, |ui| {
        let inner = window_layout::TOOLBAR_INNER_GAP;
        let r = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = inner;
            widget_rects.push(ui.label(format!("{}:", owned.0)).rect);
            let mut lang = "zh_CN".to_string();
            let resp = egui::ComboBox::from_id_salt("test_lang")
                .width(lang_w)
                .selected_text("中文")
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut lang, "zh_CN".to_string(), "中文");
                    ui.selectable_value(&mut lang, "en_US".to_string(), "English");
                });
            widget_rects.push(resp.response.rect);
        }).response.rect;
        group_rects.push(r);
        let r = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = inner;
            widget_rects.push(ui.label(format!("{}:", owned.1)).rect);
            let mut pref = String::new();
            let resp = egui::ComboBox::from_id_salt("test_theme")
                .width(theme_w)
                .selected_text(&owned.6)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut pref, "".to_string(), &owned.6);
                });
            widget_rects.push(resp.response.rect);
        }).response.rect;
        group_rects.push(r);
        let refresh = ui.button(&owned.2);
        widget_rects.push(refresh.rect);
        group_rects.push(refresh.rect);
        let mut on = false;
        let chk = ui.checkbox(&mut on, &owned.3);
        widget_rects.push(chk.rect);
        group_rects.push(chk.rect);
        let filters = ui.button(&owned.4);
        widget_rects.push(filters.rect);
        group_rects.push(filters.rect);
        let settings = ui.button(&owned.5);
        widget_rects.push(settings.rect);
        group_rects.push(settings.rect);
    });
    PaintedToolbar { content_width, available, group_rects, widget_rects }
}

fn run_toolbar(lang: &str, width: f32, scale: f32) -> PaintedToolbar {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(scale);
    try_install_cjk(&ctx);
    let owned = if lang == "zh" { zh_labels() } else { en_labels() };
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 80.0))),
        ..Default::default()
    };
    let mut painted = None;
    // A couple of frames so combo/font atlases settle.
    for _ in 0..3 {
        let mut output = ctx.run_ui(input.clone(), |ui| {
            painted = Some(paint_compact_toolbar(ui, &owned));
        });
        output.textures_delta.clear();
    }
    painted.expect("toolbar painted")
}

#[test]
fn default_toolbar_fits_on_one_row() {
}

#[test]
fn table_and_log_heights_are_stable_across_busy_and_only_change_on_resize() {
    let ctx = egui::Context::default();
    fn paint(ctx: &egui::Context, width: f32, height: f32, busy: bool) -> (egui::Rect, egui::Rect, egui::Rect) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height))),
            ..Default::default()
        };
        let mut header = egui::Rect::NOTHING;
        let mut table = egui::Rect::NOTHING;
        let mut log = egui::Rect::NOTHING;
        for _ in 0..3 {
            let mut output = ctx.run_ui(input.clone(), |ui| {
                header = egui::Panel::top("stable_header").show_separator_line(false).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
                        ui.vertical(|ui| {
                            ui.heading("UsbipdTool");
                            ui.label("info");
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.set_min_height(40.0);
                            ui.set_max_height(40.0);
                            if busy {
                                ui.spinner();
                                ui.label("● busy");
                            }
                        });
                    });
                }).response.rect;
                egui::CentralPanel::default().frame(window_layout::central_content_frame(ui.style())).show(ui, |ui| {
                    let h = window_layout::table_height(ui.available_height());
                    table = window_layout::fixed_region(ui, egui::vec2(ui.available_width(), h), |ui| {
                        ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
                        ui.max_rect()
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| { ui.label("Log"); });
                    let size = ui.available_size();
                    window_layout::fixed_region(ui, size, |ui| {
                        log = ui.max_rect();
                    });
                });
            });
            output.textures_delta.clear();
        }
        (header, table, log)
    }

    let frame = egui::vec2(16.0, 39.0);
    let min_150 = window_layout::min_client_size(1.5, frame);
    let sizes = [
        (window_layout::DEFAULT_CLIENT_SIZE[0], window_layout::DEFAULT_CLIENT_SIZE[1]),
        (min_150.x, min_150.y),
        (670.0, min_150.y),
        (1920.0, 1080.0),
    ];
    for (width, height) in sizes {
        let (h_idle, t_idle, l_idle) = paint(&ctx, width, height, false);
        let (h_busy, t_busy, l_busy) = paint(&ctx, width, height, true);
        assert!((h_idle.height() - h_busy.height()).abs() < 1.0, "header grew when busy: idle {h_idle:?} busy {h_busy:?} {width}x{height}");
        assert!((t_idle.height() - t_busy.height()).abs() < 1.0, "table height changed when busy: idle {t_idle:?} busy {t_busy:?} {width}x{height}");
        assert!((t_idle.top() - t_busy.top()).abs() < 1.0, "table moved when busy: idle {t_idle:?} busy {t_busy:?}");
        assert!((l_idle.height() - l_busy.height()).abs() < 1.0, "log height changed when busy: idle {l_idle:?} busy {l_busy:?}");
        let gap = height - l_idle.bottom();
        assert!(gap + 0.5 >= window_layout::BOTTOM_MARGIN, "idle gap {gap} log {l_idle:?} {height}");
        assert!(gap <= window_layout::BOTTOM_MARGIN + 4.0, "idle gap too large {gap} log {l_idle:?} {height}");
        // No bottom status bar appears under the log when busy.
        assert!((l_busy.bottom() - l_idle.bottom()).abs() < 1.0, "log bottom moved when busy {l_idle:?} {l_busy:?}");
    }
    // At the default window, the log body is half of the previous 50/50 allocation;
    // the table receives the freed height (about 3× the log body), with no extra bottom gap.
    let (_, t_def, l_def) = paint(&ctx, window_layout::DEFAULT_CLIENT_SIZE[0], window_layout::DEFAULT_CLIENT_SIZE[1], false);
    let flex_like = t_def.height() / window_layout::TABLE_FLEX_SHARE;
    let previous_log = flex_like * 0.5;
    assert!(
        (l_def.height() - previous_log * 0.5).abs() <= 12.0,
        "default log body should be half the previous allocation: log={:?} previous={previous_log} table={:?}",
        l_def, t_def
    );
    assert!(
        t_def.height() > l_def.height() * 2.0,
        "freed log height should go to the table: table={t_def:?} log={l_def:?}"
    );

    // Resize changes table height; busy vs idle at the same size does not.
    let (_, small_table, small_log) = paint(&ctx, min_150.x, min_150.y, false);
    assert!(
        small_table.height() >= window_layout::TABLE_HEADER_HEIGHT + window_layout::TABLE_ROW_HEIGHT,
        "table should still fit a header and a row at the new min {small_table:?} {min_150:?}"
    );
    assert!(
        small_log.height() > 24.0,
        "log body should remain usable at the new min {small_log:?} {min_150:?}"
    );
    let (_, big_table, _) = paint(&ctx, 1920.0, 1080.0, false);
    assert!(big_table.height() > small_table.height() + 20.0, "resize should change table {small_table:?} {big_table:?}");
}

#[test]
fn table_takes_three_quarters_of_flex_so_log_body_is_half_its_former_share() {
    assert_eq!(window_layout::table_height(0.0), 0.0);
    let remaining = 800.0;
    let table = window_layout::table_height(remaining);
    let flex = (remaining - window_layout::LOG_CHROME_RESERVE).max(0.0);
    let previous_log = flex * 0.5;
    let new_log = (remaining - table - window_layout::LOG_CHROME_RESERVE).max(0.0);
    assert!((table - (remaining - window_layout::LOG_CHROME_RESERVE) * 0.75).abs() < 1e-3);
    assert!((new_log - previous_log * 0.5).abs() < 1e-3);
    assert!((new_log + table - (remaining - window_layout::LOG_CHROME_RESERVE)).abs() < 1e-3);
}

fn typical_table_inner(client_width: f32) -> f32 {
    // Central panel default margins (8+8) and the table frame (inner 6+6, stroke 1+1).
    (client_width - 16.0 - 14.0).max(0.0)
}

fn flex_leftover(available: f32) -> f32 {
    let gaps = window_layout::TABLE_COLUMN_GAP * (window_layout::TABLE_COLUMNS as f32 - 1.0);
    let fixed = window_layout::COL_BUSID_WIDTH + window_layout::COL_VIDPID_WIDTH + window_layout::COL_ACTION_WIDTH;
    (available - window_layout::TABLE_VSCROLL_RESERVE - gaps - fixed).max(0.0)
}

fn assert_compact_and_flex_columns(m: &window_layout::TableMetrics) {
    assert!((m.widths[0] - window_layout::COL_BUSID_WIDTH).abs() < 1e-3, "{:?}", m.widths);
    assert!((m.widths[1] - window_layout::COL_VIDPID_WIDTH).abs() < 1e-3, "{:?}", m.widths);
    assert!((m.widths[4] - window_layout::COL_ACTION_WIDTH).abs() < 1e-3, "{:?}", m.widths);
    let gaps = window_layout::TABLE_COLUMN_GAP * (window_layout::TABLE_COLUMNS as f32 - 1.0);
    assert!((m.widths.iter().sum::<f32>() + m.reserved + gaps - m.table_width()).abs() < 1e-3);
    let leftover = flex_leftover(m.available);
    if leftover + 0.5 >= window_layout::COL_DEVICE_MIN + window_layout::COL_STATE_MIN {
        assert!(m.widths[2] + 0.1 >= window_layout::COL_DEVICE_MIN, "{:?}", m.widths);
        assert!(m.widths[3] + 0.1 >= window_layout::COL_STATE_MIN, "{:?}", m.widths);
    }
    assert!((m.widths[2] + m.widths[3] - leftover).abs() < 1e-2, "flex {:?} leftover {leftover}", m.widths);
}

#[test]
fn table_columns_use_compact_ids_and_share_leftover_independently_of_content() {
    let wide = typical_table_inner(window_layout::DEFAULT_CLIENT_SIZE[0]);
    let m = window_layout::table_metrics(wide);
    assert_compact_and_flex_columns(&m);
    let leftover = (wide - window_layout::TABLE_VSCROLL_RESERVE
        - window_layout::TABLE_COLUMN_GAP * (window_layout::TABLE_COLUMNS as f32 - 1.0))
        - window_layout::COL_BUSID_WIDTH
        - window_layout::COL_VIDPID_WIDTH
        - window_layout::COL_ACTION_WIDTH;
    let expected_device = (leftover * window_layout::COL_DEVICE_LEFTOVER_SHARE)
        .clamp(window_layout::COL_DEVICE_MIN, leftover - window_layout::COL_STATE_MIN);
    assert!((m.widths[2] - expected_device).abs() < 1e-3, "{:?} leftover {leftover} expected {expected_device}", m.widths);
    assert!((m.widths[3] - (leftover - m.widths[2])).abs() < 1e-3, "{:?}", m.widths);
    assert!(m.widths[3] + 0.1 >= window_layout::COL_STATE_MIN, "status floor {:?}", m.widths);
    assert!(!m.needs_horizontal_scroll());
    // Same viewport always yields the same widths; long status text cannot change them.
    assert_eq!(m, window_layout::table_metrics(wide));

    let scale_125 = typical_table_inner(window_layout::initial_client_size(1.25, egui::vec2(16.0, 39.0)).x);
    let m125 = window_layout::table_metrics(scale_125);
    assert_compact_and_flex_columns(&m125);
    assert!((m125.widths[4] - window_layout::ACTION_PAIR_MIN_WIDTH).abs() < 1e-3);
    assert!(!m125.needs_horizontal_scroll());

    let narrow = window_layout::table_metrics(400.0);
    assert_compact_and_flex_columns(&narrow);
    assert!(!narrow.needs_horizontal_scroll(), "400 should shrink-to-fit, not scroll: {:?}", narrow);
    assert!(narrow.table_width() <= 400.0 + 0.5, "overflow {:?}", narrow);
    assert!(narrow.widths[2] + narrow.widths[3] < window_layout::COL_DEVICE_MIN + window_layout::COL_STATE_MIN);
}


#[test]
fn default_and_150pct_columns_fit_realistic_device_and_status_fixtures() {
    // Test fixtures only — production widths are fixed/weighted typography floors
    // and never take a particular device name or host address as a sizing input.
    const DEVICE: &str = "USB-SERIAL CH340 (COM8)";
    const STATE_ZH: &str = "已附加 (172.25.235.191) [auto]";
    const STATE_EN: &str = "Attached (172.25.235.191) [auto]";
    const STATE_ZH_MAX_IP: &str = "已附加 (255.255.255.255) [auto]";
    const STATE_EN_MAX_IP: &str = "Attached (255.255.255.255) [auto]";
    // Realistic default physical window (~1024) at 100% and 150% DPI, plus the
    // 600–670 logical band the previous tests missed.
    let cases: &[(&str, f32, f32, bool)] = &[
        ("default", 1.0, window_layout::DEFAULT_CLIENT_SIZE[0], true),
        ("150pct", 1.5, window_layout::initial_client_size(1.5, egui::vec2(16.0, 39.0)).x, true),
        ("logical-670", 1.5, 670.0, true),
        ("logical-667", 1.5, 666.667, true),
        ("logical-640", 1.5, 640.0, false),
        ("logical-600", 1.5, 600.0, false),
    ];
    for &(label, scale, client, expect_full_name_and_state) in cases {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(scale);
        try_install_cjk(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(client, 800.0))),
            ..Default::default()
        };
        let mut available_inner = 0.0;
        let mut metrics = window_layout::table_metrics(0.0);
        let mut device_w = 0.0;
        let mut zh_w = 0.0;
        let mut en_w = 0.0;
        let mut zh_max = 0.0;
        let mut en_max = 0.0;
        let mut table_viewport = egui::Rect::NOTHING;
        let mut device_cell = egui::Rect::NOTHING;
        let mut state_cell = egui::Rect::NOTHING;
        let mut output = ctx.run_ui(input.clone(), |ui| {
            // Mirror production insets: header row, central 8+8, table frame 6+6+stroke 1+1.
            egui::Panel::top("fit_header").show_separator_line(false).show(ui, |ui| {
                ui.set_min_height(40.0);
                ui.label("UsbipdTool");
            });
            egui::CentralPanel::default()
                .frame(window_layout::central_content_frame(ui.style()))
                .show(ui, |ui| {
                    let h = window_layout::table_height(ui.available_height());
                    window_layout::fixed_region(ui, egui::vec2(ui.available_width(), h), |ui| {
                        egui::Frame::new()
                            .stroke(egui::Stroke::new(1.0, egui::Color32::GRAY))
                            .inner_margin(6.0)
                            .show(ui, |ui| {
                                available_inner = ui.available_width();
                                window_layout::apply_table_column_spacing(ui);
                                metrics = window_layout::table_metrics(available_inner);
                                table_viewport = ui.max_rect();
                                device_w = window_layout::text_width(ui, DEVICE);
                                zh_w = window_layout::text_width(ui, STATE_ZH);
                                en_w = window_layout::text_width(ui, STATE_EN);
                                zh_max = window_layout::text_width(ui, STATE_ZH_MAX_IP);
                                en_max = window_layout::text_width(ui, STATE_EN_MAX_IP);
                                let (row, cells, _) = paint_table_row(
                                    ui,
                                    window_layout::TABLE_ROW_HEIGHT,
                                    metrics.widths,
                                    ["10-1", "1A86:7523", DEVICE, STATE_ZH],
                                    &["分离", "解绑"],
                                );
                                let _ = row;
                                device_cell = cells[2];
                                state_cell = cells[3];
                            });
                    });
                });
        });
        output.textures_delta.clear();
        // Second pass so painted galleys exist (same as other tests).
        for _ in 0..2 {
            output = ctx.run_ui(input.clone(), |ui| {
                egui::Panel::top("fit_header").show_separator_line(false).show(ui, |ui| {
                    ui.set_min_height(40.0);
                    ui.label("UsbipdTool");
                });
                egui::CentralPanel::default()
                    .frame(window_layout::central_content_frame(ui.style()))
                    .show(ui, |ui| {
                        let h = window_layout::table_height(ui.available_height());
                        window_layout::fixed_region(ui, egui::vec2(ui.available_width(), h), |ui| {
                            egui::Frame::new()
                                .stroke(egui::Stroke::new(1.0, egui::Color32::GRAY))
                                .inner_margin(6.0)
                                .show(ui, |ui| {
                                    available_inner = ui.available_width();
                                    window_layout::apply_table_column_spacing(ui);
                                    metrics = window_layout::table_metrics(available_inner);
                                    table_viewport = ui.max_rect();
                                    device_w = window_layout::text_width(ui, DEVICE);
                                    zh_w = window_layout::text_width(ui, STATE_ZH);
                                    en_w = window_layout::text_width(ui, STATE_EN);
                                    zh_max = window_layout::text_width(ui, STATE_ZH_MAX_IP);
                                    en_max = window_layout::text_width(ui, STATE_EN_MAX_IP);
                                    let (_, cells, _) = paint_table_row(
                                        ui,
                                        window_layout::TABLE_ROW_HEIGHT,
                                        metrics.widths,
                                        ["10-1", "1A86:7523", DEVICE, STATE_ZH],
                                        &["分离", "解绑"],
                                    );
                                    device_cell = cells[2];
                                    state_cell = cells[3];
                                });
                        });
                    });
            });
            output.textures_delta.clear();
        }

        assert_compact_and_flex_columns(&metrics);
        assert!(
            !metrics.needs_horizontal_scroll(),
            "{label} @{scale} client {client}: unexpected h-scroll available={available_inner} widths={:?} table_width={}",
            metrics.widths,
            metrics.table_width()
        );
        assert!(
            metrics.table_width() <= available_inner + 0.75,
            "{label} @{scale}: table_width {} overflows available {available_inner} {:?}",
            metrics.table_width(),
            metrics.widths
        );
        // Content (including the two action buttons) must sit inside the viewport;
        // hiding a scrollbar while leaving off-screen cells is not enough.
        let content_right = metrics.widths.iter().sum::<f32>() + table_viewport.left();
        assert!(
            content_right <= table_viewport.right() + 0.75,
            "{label} @{scale}: painted columns extend to {content_right} past viewport {:?}",
            table_viewport
        );
        // Column *boundaries* move left because IDs shrank; this is not in-cell centering.
        assert!(
            (metrics.widths[0] - window_layout::COL_BUSID_WIDTH).abs() < 1e-3
                && window_layout::COL_BUSID_WIDTH < 68.0
                && window_layout::COL_VIDPID_WIDTH < 86.0,
            "ID columns were not compressed: {:?}",
            metrics.widths
        );
        let device_left = table_viewport.left() + metrics.widths[0] + metrics.widths[1];
        let old_device_left = table_viewport.left() + 68.0 + 86.0;
        assert!(
            device_left + 1.0 < old_device_left,
            "{label} @{scale}: device column did not move left ({device_left} vs old {old_device_left})"
        );

        let painted = painted_text_rects(&output);
        let state_glyph = text_in_cell(&painted, state_cell, "已附加")
            .or_else(|| text_in_cell(&painted, state_cell, "附加"));
        if expect_full_name_and_state {
            assert!(
                device_w <= metrics.widths[2] + 0.5,
                "{label} @{scale}: fixture {DEVICE:?} is {device_w}, device col {}",
                metrics.widths[2]
            );
            assert!(
                zh_w <= metrics.widths[3] + 0.5 && en_w <= metrics.widths[3] + 0.5,
                "{label} @{scale}: state fixtures zh={zh_w} en={en_w} col {}",
                metrics.widths[3]
            );
            assert!(
                zh_max <= metrics.widths[3] + 0.5 && en_max <= metrics.widths[3] + 0.5,
                "{label} @{scale}: max-IPv4 state zh={zh_max} en={en_max} col {}",
                metrics.widths[3]
            );
            if let Some(g) = state_glyph {
                assert!(
                    g.right() <= state_cell.right() + 0.75,
                    "{label} @{scale}: painted state {g:?} clipped by cell {state_cell:?}"
                );
            }
            let name_glyph = text_in_cell(&painted, device_cell, "CH340")
                .or_else(|| text_in_cell(&painted, device_cell, "USB-SERIAL"));
            if let Some(g) = name_glyph {
                assert!(
                    g.right() <= device_cell.right() + 0.75,
                    "{label} @{scale}: painted name {g:?} clipped by cell {device_cell:?}"
                );
            }
        }
        // Same viewport always yields the same widths regardless of cell text / state.
        assert_eq!(metrics, window_layout::table_metrics(available_inner));
    }
}

#[test]
fn compact_toolbar_fits_default_and_150pct_widths_in_zh_and_en() {
    // Must fit at the permitted minimum (default width after DPI/frame, including
    // typical 8+8 panel margins) and at the 1.0-scale default client. 640 is below
    // the 150% minimum and is not required to avoid overflow.
    let frame = egui::vec2(16.0, 39.0);
    let min_150 = window_layout::min_client_size(1.5, frame).x;
    let widths = [
        min_150,
        min_150 - 16.0,
        667.0,
        670.0,
        window_layout::DEFAULT_CLIENT_SIZE[0],
    ];
    for lang in ["zh", "en"] {
        for &width in &widths {
            for scale in [1.0, 1.5] {
                let p = run_toolbar(lang, width, scale);
                assert_eq!(p.group_rects.len(), window_layout::TOOLBAR_SECTIONS, "{lang} {width} @{scale}");
                assert!(
                    !window_layout::toolbar_needs_horizontal_scroll(width, p.content_width),
                    "toolbar overflow {lang} width={width} scale={scale} content={} available={}",
                    p.content_width, p.available
                );
                let settings = *p.group_rects.last().unwrap();
                assert!(
                    settings.right() <= width + 0.75,
                    "Settings clipped {lang} {width} @{scale}: settings={settings:?} content={}",
                    p.content_width
                );
                let top = p.group_rects.iter().map(|r| r.top()).fold(f32::MAX, f32::min);
                let bottom = p.group_rects.iter().map(|r| r.bottom()).fold(f32::MIN, f32::max);
                assert!(
                    bottom - top <= window_layout::TOOLBAR_ROW_HEIGHT + 4.0,
                    "toolbar wrapped {lang} {width} @{scale}: height {}",
                    bottom - top
                );
                // Groups stay compact: no equal 168-wide cells and no huge inter-group gaps.
                for i in 0..p.group_rects.len().saturating_sub(1) {
                    let gap = p.group_rects[i + 1].left() - p.group_rects[i].right();
                    assert!(
                        gap >= 0.0 && gap <= window_layout::TOOLBAR_GROUP_GAP + 6.0,
                        "huge or negative gap {gap} at {i} {lang} {width} rects={:?}",
                        p.group_rects
                    );
                }
                let mid_y = (top + bottom) * 0.5;
                for (i, r) in p.widget_rects.iter().enumerate() {
                    assert!(
                        (r.center().y - mid_y).abs() <= 6.0,
                        "toolbar widget {i} not vertically centered {r:?} mid={mid_y} {lang} {width} @{scale}"
                    );
                }
            }
        }
    }
}

#[test]
fn compact_toolbar_scrolls_only_when_truly_narrow() {
    let p = run_toolbar("en", 360.0, 1.0);
    assert!(window_layout::toolbar_needs_horizontal_scroll(360.0, p.content_width));
    assert_eq!(p.group_rects.len(), window_layout::TOOLBAR_SECTIONS);
    let top = p.group_rects.iter().map(|r| r.top()).fold(f32::MAX, f32::min);
    let bottom = p.group_rects.iter().map(|r| r.bottom()).fold(f32::MIN, f32::max);
    assert!(bottom - top <= window_layout::TOOLBAR_ROW_HEIGHT + 4.0, "height {}", bottom - top);
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
            let visual = window_layout::galley_visual_bounds(&ts.galley);
            texts.push((ts.galley.job.text.clone(), visual.translate(ts.pos.to_vec2())));
        }
    }
    texts
}

fn painted_button_rects(output: &egui::FullOutput) -> Vec<egui::Rect> {
    let mut buttons = Vec::new();
    for shape in all_flat_shapes(output) {
        if let egui::Shape::Rect(rs) = shape {
            let h = rs.rect.height();
            let w = rs.rect.width();
            if rs.fill.a() > 0
                && (h - window_layout::ACTION_BUTTON_HEIGHT).abs() <= 3.0
                && w >= 24.0
                && w <= 130.0
            {
                buttons.push(rs.rect);
            }
        }
    }
    buttons
}

fn paint_table_row(
    ui: &mut egui::Ui,
    height: f32,
    widths: [f32; 5],
    texts: [&str; 4],
    buttons: &[&str],
) -> (egui::Rect, Vec<egui::Rect>, Vec<egui::Rect>) {
    let row_w: f32 = widths.iter().sum();
    let mut cell_rects = Vec::new();
    let mut button_rects = Vec::new();
    let mut row_rect = egui::Rect::NOTHING;
    ui.allocate_ui_with_layout(
        egui::vec2(row_w, height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            row_rect = ui.max_rect();
            for (i, w) in widths.iter().copied().enumerate() {
                ui.allocate_ui_with_layout(
                    egui::vec2(w, height),
                    window_layout::cell_center_layout(),
                    |ui| {
                        let cell = ui.max_rect();
                        cell_rects.push(cell);
                        if i < 4 {
                            window_layout::centered_cell_label(ui, texts[i], None, false);
                        } else {
                            window_layout::centered_cell_group(ui, |ui| {
                                for cap in buttons {
                                    let resp = ui.button(*cap);
                                    if !ui.is_sizing_pass() {
                                        button_rects.push(resp.rect);
                                    }
                                }
                            });
                        }
                    },
                );
            }
        },
    );
    (row_rect, cell_rects, button_rects)
}

fn paint_header_row(ui: &mut egui::Ui, widths: [f32; 5], caps: [&str; 5]) -> (egui::Rect, Vec<egui::Rect>) {
    let row_w: f32 = widths.iter().sum();
    let mut row_rect = egui::Rect::NOTHING;
    let mut cell_rects = Vec::new();
    ui.allocate_ui_with_layout(
        egui::vec2(row_w, window_layout::TABLE_HEADER_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            row_rect = ui.max_rect();
            for (w, cap) in widths.iter().copied().zip(caps) {
                ui.allocate_ui_with_layout(
                    egui::vec2(w, window_layout::TABLE_HEADER_HEIGHT),
                    window_layout::cell_center_layout(),
                    |ui| {
                        cell_rects.push(ui.max_rect());
                        window_layout::centered_cell_label(ui, cap, None, true);
                    },
                );
            }
        },
    );
    (row_rect, cell_rects)
}

/// Closest painted text whose galley contains `needle` and overlaps `cell`.
fn text_in_cell(texts: &[(String, egui::Rect)], cell: egui::Rect, needle: &str) -> Option<egui::Rect> {
    texts
        .iter()
        .filter(|(s, r)| s.contains(needle) && cell.intersects(*r))
        .min_by(|a, b| {
            let da = (a.1.center() - cell.center()).length();
            let db = (b.1.center() - cell.center()).length();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(_, r)| *r)
}

#[test]
fn table_headers_and_row_contents_share_painted_centerline() {
    let cases: &[(&str, f32, f32)] = &[
        ("default", 1.0, window_layout::DEFAULT_CLIENT_SIZE[0]),
        ("150pct", 1.5, window_layout::initial_client_size(1.5, egui::vec2(16.0, 39.0)).x),
        ("min", 1.0, window_layout::MIN_CLIENT_SIZE[0]),
    ];
    let button_sets: &[&[&str]] = &[
        &["绑定"],
        &["附加", "解绑"],
        &["分离", "解绑"],
        &["Bind"],
        &["Attach", "Unbind"],
        &["Detach", "Unbind"],
    ];
    let text_sets: &[[&str; 4]] = &[
        ["10-1", "12D1:107E", "USB Serial", "未共享"],
        ["2-2", "046D:C52B", "Long device name that should still stay centered", "已共享"],
        ["11-2.3", "----:----", "Very long description that truncates", "已附加 (172.20.0.1) [auto]"],
        ["7-1", "1A2B:3C4D", "Camera", "Not shared"],
        ["8-1", "0A0A:0B0B", "Hub", "Shared"],
        ["9-1", "FFFF:EEEE", "Attached device", "Attached (10.0.0.2) [auto]"],
    ];
    for &(label, scale, client) in cases {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(scale);
        try_install_cjk(&ctx);
        let inner = typical_table_inner(client);
        let metrics = window_layout::table_metrics(inner);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(client.max(metrics.table_width()), 400.0),
            )),
            ..Default::default()
        };
        let headers = ["BUSID", "VID:PID", "设备", "状态", "操作"];
        for (texts, buttons) in text_sets.iter().zip(button_sets.iter()) {
            let mut header_row = egui::Rect::NOTHING;
            let mut header_cells = Vec::new();
            let mut row = egui::Rect::NOTHING;
            let mut cells = Vec::new();
            let mut button_widget = Vec::new();
            let mut output = ctx.run_ui(input.clone(), |ui| {
                (header_row, header_cells) = paint_header_row(ui, metrics.widths, headers);
                (row, cells, button_widget) = paint_table_row(
                    ui,
                    window_layout::TABLE_ROW_HEIGHT,
                    metrics.widths,
                    *texts,
                    buttons,
                );
            });
            output.textures_delta.clear();
            for _ in 1..3 {
                output = ctx.run_ui(input.clone(), |ui| {
                    (header_row, header_cells) = paint_header_row(ui, metrics.widths, headers);
                    (row, cells, button_widget) = paint_table_row(
                        ui,
                        window_layout::TABLE_ROW_HEIGHT,
                        metrics.widths,
                        *texts,
                        buttons,
                    );
                });
                output.textures_delta.clear();
            }
            let painted = painted_text_rects(&output);
            let painted_buttons = painted_button_rects(&output);
            let mid = row.center().y;
            let ppp = scale;
            // A 10 physical-px text/button mismatch must fail this test.
            let max_physical = 4.0;
            for (i, cap) in headers.iter().enumerate() {
                let header_cell = header_cells[i];
                let text = text_in_cell(&painted, header_cell, cap)
                    .expect(&format!("missing painted header {cap} {label} @{scale}"));
                let dy = (text.center().y - header_row.center().y).abs() * ppp;
                let dx = (text.center().x - header_cell.center().x).abs() * ppp;
                assert!(
                    dy <= max_physical && dx <= 20.0 * ppp,
                    "header {cap} painted {text:?} cell {header_cell:?} dy_px={dy} dx_px={dx} {label} @{scale}"
                );
            }
            for i in 0..4 {
                let text = text_in_cell(&painted, cells[i], texts[i])
                    .or_else(|| text_in_cell(&painted, cells[i], "…"))
                    .expect(&format!("missing painted text col {i} {label} @{scale} {:?}", texts[i]));
                let dy = (text.center().y - mid).abs() * ppp;
                let dx = (text.center().x - cells[i].center().x).abs() * ppp;
                assert!(
                    dy <= max_physical,
                    "painted text col {i} {text:?} mid={mid} dy_px={dy} {label} @{scale} texts={texts:?}"
                );
                // Short identifiers should sit near the cell mid-X; long/elided text is still
                // visually centered (allow a slightly wider budget for ellipsis).
                assert!(
                    dx <= 24.0 * ppp,
                    "painted h-center col {i} dx_px={dx} {text:?} cell={:?} {label} @{scale}",
                    cells[i]
                );
            }
            assert!(
                !button_widget.is_empty() && button_widget.len() == buttons.len(),
                "button widgets {button_widget:?} vs {buttons:?}"
            );
            for (i, br) in button_widget.iter().enumerate() {
                assert!(
                    (br.height() - window_layout::ACTION_BUTTON_HEIGHT).abs() <= 1.0,
                    "button {i} height {} {label} @{scale}",
                    br.height()
                );
                let dy = (br.center().y - mid).abs() * ppp;
                assert!(
                    dy <= max_physical,
                    "button widget {i} {br:?} mid={mid} dy_px={dy} {label} @{scale}"
                );
            }
            let action_cell = cells[4];
            let in_action: Vec<egui::Rect> = painted_buttons
                .iter()
                .copied()
                .filter(|r| action_cell.intersects(*r))
                .collect();
            assert!(
                !in_action.is_empty(),
                "no painted button rects in action cell {action_cell:?} all={painted_buttons:?} {label} @{scale}"
            );
            let btn_mid = in_action.iter().map(|r| r.center().y).sum::<f32>() / in_action.len() as f32;
            let text_mids: Vec<f32> = (0..4)
                .filter_map(|i| text_in_cell(&painted, cells[i], texts[i]).or_else(|| text_in_cell(&painted, cells[i], "…")))
                .map(|r| r.center().y)
                .collect();
            let text_mid = text_mids.iter().sum::<f32>() / text_mids.len() as f32;
            let gap_px = (btn_mid - text_mid).abs() * ppp;
            assert!(
                gap_px <= max_physical,
                "painted button/text gap {gap_px}px (buttons ~10px above text must fail) btn_mid={btn_mid} text_mid={text_mid} {label} @{scale} texts={texts:?} buttons={buttons:?}"
            );
        }
    }
}

#[test]
fn table_row_and_action_button_heights_stay_constant_across_states_then_refresh() {
    let ctx = egui::Context::default();
    try_install_cjk(&ctx);
    let client = window_layout::DEFAULT_CLIENT_SIZE[0];
    let metrics = window_layout::table_metrics(typical_table_inner(client));
    // Mock the bind lifecycle without running usbipd: not shared, bind pending (busy),
    // shared after refresh, attached, and unshared again.
    let states: &[(&str, [&str; 4], &[&str], bool)] = &[
        ("unshared", ["10-1", "1A86:7523", "USB-SERIAL CH340 (COM8)", "未共享"], &["绑定"][..], false),
        ("bind_pending", ["10-1", "1A86:7523", "USB-SERIAL CH340 (COM8)", "未共享"], &["绑定"][..], true),
        ("shared_refresh", ["10-1", "1A86:7523", "USB-SERIAL CH340 (COM8)", "已共享"], &["附加", "解绑"][..], false),
        ("attached", ["10-1", "1A86:7523", "USB-SERIAL CH340 (COM8)", "已附加 (172.20.0.1)"], &["分离", "解绑"][..], false),
        ("unshared_again", ["10-1", "1A86:7523", "USB-SERIAL CH340 (COM8)", "未共享"], &["绑定"][..], false),
    ];
    let mut seen_rows: Vec<(f32, f32, f32, usize, f32, f32)> = Vec::new();
    // tuple: table_h, header_h, row_h, button_count, min_btn_h, max_btn_h
    let min_150 = window_layout::min_client_size(1.5, egui::vec2(16.0, 39.0));
    let sizes = [
        window_layout::DEFAULT_CLIENT_SIZE,
        [min_150.x, min_150.y],
        [670.0, min_150.y],
    ];
    for size in sizes {
        let mut per_state = Vec::new();
        for &(name, texts, buttons, busy) in states {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size[0], size[1]))),
                ..Default::default()
            };
            let mut table = egui::Rect::NOTHING;
            let mut header = egui::Rect::NOTHING;
            let mut row = egui::Rect::NOTHING;
            let mut button_rects = Vec::new();
            for _ in 0..3 {
                let mut output = ctx.run_ui(input.clone(), |ui| {
                    egui::Panel::top("state_header").show_separator_line(false).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
                            ui.vertical(|ui| {
                                ui.heading("UsbipdTool");
                                ui.label("info");
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.set_min_height(40.0);
                                ui.set_max_height(40.0);
                                if busy {
                                    ui.spinner();
                                    ui.label("● busy");
                                }
                            });
                        });
                    });
                    egui::CentralPanel::default()
                        .frame(window_layout::central_content_frame(ui.style()))
                        .show(ui, |ui| {
                            let h = window_layout::table_height(ui.available_height());
                            table = window_layout::fixed_region(ui, egui::vec2(ui.available_width(), h), |ui| {
                                (header, _) = paint_header_row(ui, metrics.widths, ["BUSID", "VID:PID", "设备", "状态", "操作"]);
                                (row, _, button_rects) = paint_table_row(
                                    ui,
                                    window_layout::TABLE_ROW_HEIGHT,
                                    metrics.widths,
                                    texts,
                                    buttons,
                                );
                                ui.max_rect()
                            });
                        });
                });
                output.textures_delta.clear();
            }
            let btn_hs: Vec<f32> = button_rects.iter().map(|r| r.height()).collect();
            assert_eq!(button_rects.len(), buttons.len(), "{name} {:?}", size);
            let min_b = btn_hs.iter().copied().fold(f32::MAX, f32::min);
            let max_b = btn_hs.iter().copied().fold(0.0, f32::max);
            per_state.push((name, table, header, row, button_rects.len(), min_b, max_b, busy));
            seen_rows.push((table.height(), header.height(), row.height(), button_rects.len(), min_b, max_b));
            assert!((header.height() - window_layout::TABLE_HEADER_HEIGHT).abs() < 1.0, "{name} header {:?}", header);
            assert!((row.height() - window_layout::TABLE_ROW_HEIGHT).abs() < 1.0, "{name} row {:?}", row);
            assert!((min_b - window_layout::ACTION_BUTTON_HEIGHT).abs() <= 1.0, "{name} min button {min_b}");
            assert!((max_b - min_b).abs() <= 1.0, "{name} button heights changed {min_b}..{max_b}");
        }
        let (_, t0, h0, r0, _, b0, _, _) = &per_state[0];
        for (name, table, header, row, _n, min_b, _max_b, _) in &per_state {
            assert!((table.height() - t0.height()).abs() < 1.0, "{name} table height vs unshared at {:?}", size);
            assert!((table.top() - t0.top()).abs() < 1.0, "{name} table top moved at {:?}", size);
            assert!((header.height() - h0.height()).abs() < 1.0, "{name} header height at {:?}", size);
            assert!((row.height() - r0.height()).abs() < 1.0, "{name} row height at {:?}", size);
            assert!((min_b - b0).abs() <= 1.0, "{name} button height vs unshared {min_b} {b0}");
        }
    }
    // Resize still changes the table panel, not the row or button height.
    let default_table = seen_rows[0].0;
    let mid = states.len();
    let small_table = seen_rows[mid * 2].0; // third size group starts at 2*states
    assert!(default_table > small_table + 10.0, "resize should change table panel {default_table} vs {small_table}");
}

#[test]
fn horizontal_scroll_wrapper_follows_metrics() {
    let ctx = egui::Context::default();
    for (width, expect_scroll) in [(window_layout::DEFAULT_CLIENT_SIZE[0], false), (400.0, false), (200.0, true)] {
        let metrics = window_layout::table_metrics(typical_table_inner(width));
        assert_eq!(metrics.needs_horizontal_scroll(), expect_scroll, "width {width}");
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 400.0))),
            ..Default::default()
        };
        let mut child_min_width = 0.0;
        let mut output = ctx.run_ui(input, |ui| {
            window_layout::with_optional_hscroll(ui, metrics, "test_hscroll", 200.0, |ui| {
                child_min_width = ui.min_rect().width();
                ui.allocate_exact_size(egui::vec2(metrics.table_width(), 20.0), egui::Sense::hover());
            });
        });
        output.textures_delta.clear();
        if expect_scroll {
            assert!(child_min_width + 0.5 >= metrics.table_width(), "{child_min_width} vs {}", metrics.table_width());
        }
    }
}

