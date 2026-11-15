//! Responsive window regions. Dimensions are egui points unless explicitly physical.
use super::egui;

/// Decorated window target in physical pixels (title bar included).
pub const DECORATED_WINDOW_SIZE: [f32; 2] = [1024.0, 760.0];
/// Client area at 1.0 scale with a 16×39 point window frame (1024-16, 760-39).
pub const DEFAULT_CLIENT_SIZE: [f32; 2] = [1008.0, 721.0];
/// 1.0-scale minimum height equals the default client height. Live minima at
/// other scales use [`min_client_size`] (same DPI / frame conversion as width).
pub const MIN_CLIENT_HEIGHT: f32 = DEFAULT_CLIENT_SIZE[1];
/// 1.0-scale client minimum, equal to the default on both axes.
pub const MIN_CLIENT_SIZE: [f32; 2] = [DEFAULT_CLIENT_SIZE[0], MIN_CLIENT_HEIGHT];
/// Tight inset under the log; always applied, independent of busy or status text.
pub const BOTTOM_MARGIN: f32 = 8.0;

/// Device-table columns: compact fixed BUSID / VID:PID (tight cell padding, no
/// per-device sample strings), a stable action column, and leftover width split
/// between the device name and status. Status has a typography floor sized for a
/// short localized state, an IPv4-length address, and an optional `[auto]` tag
/// at the default body size — not a particular host or device. Widths depend
/// only on the table viewport, never on the current cell text.
pub const TABLE_COLUMNS: usize = 5;
/// Compact pair of action buttons (padding tightened in the cell) plus spacing.
pub const ACTION_PAIR_MIN_WIDTH: f32 = 148.0;
/// Tight width for compact busids such as "10-1" or "11-2.3.4", plus a sliver of padding.
pub const COL_BUSID_WIDTH: f32 = 54.0;
/// Tight width for a 9-character VID:PID such as "12D1:107E", plus a sliver of padding.
pub const COL_VIDPID_WIDTH: f32 = 72.0;
pub const COL_ACTION_WIDTH: f32 = ACTION_PAIR_MIN_WIDTH;
/// Preferred floors for the flexible columns at a default-sized window.
/// Narrower viewports shrink these to fit (ellipsis) instead of overflowing.
pub const COL_DEVICE_MIN: f32 = 160.0;
pub const COL_STATE_MIN: f32 = 204.0;
/// Share of leftover width given to the device column (rest to status), after floors.
pub const COL_DEVICE_LEFTOVER_SHARE: f32 = 0.50;
/// Gap between TableBuilder columns; must match [`apply_table_column_spacing`].
pub const TABLE_COLUMN_GAP: f32 = 0.0;
/// Floating egui scrollbars allocate no reserved width (`ScrollStyle::floating`).
pub const TABLE_VSCROLL_RESERVE: f32 = 0.0;
/// Fixed body-row height; all cell contents share this row's vertical centerline.
pub const TABLE_ROW_HEIGHT: f32 = 34.0;
/// Fixed header-row height; header labels are centered on this row's centerline.
pub const TABLE_HEADER_HEIGHT: f32 = 26.0;
/// Action buttons use this height in every device state (one or two buttons, idle or busy).
pub const ACTION_BUTTON_HEIGHT: f32 = 22.0;

pub const TOOLBAR_SECTIONS: usize = 6;
/// Gap between the six compact toolbar groups (not equal-width cells).
pub const TOOLBAR_GROUP_GAP: f32 = 8.0;
/// Gap between a group label and its control.
pub const TOOLBAR_INNER_GAP: f32 = 4.0;
pub const TOOLBAR_ROW_HEIGHT: f32 = 32.0;


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableMetrics {
    pub available: f32,
    pub reserved: f32,
    pub widths: [f32; TABLE_COLUMNS],
}

impl TableMetrics {
    pub fn table_width(self) -> f32 {
        self.widths.iter().sum::<f32>() + self.reserved + column_gaps()
    }

    pub fn needs_horizontal_scroll(self) -> bool {
        self.table_width() > self.available + 0.5
    }
}

/// Inter-column spacing used by `egui_extras::TableBuilder` (it adds
/// `item_spacing.x` between exact columns). Keep this at [`TABLE_COLUMN_GAP`].
pub fn apply_table_column_spacing(ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing.x = TABLE_COLUMN_GAP;
}

fn column_gaps() -> f32 {
    TABLE_COLUMN_GAP * (TABLE_COLUMNS.saturating_sub(1) as f32)
}

/// Column widths from the table inner viewport only — never from cell text.
/// Fixed ID and action columns stay compact; leftover is split between device
/// and status (status keeps its typography floor when there is room). If the
/// leftover is tighter than both floors, the flex columns shrink to fit so the
/// table does not overflow or sprout a horizontal scrollbar.
pub fn table_metrics(available_inner_width: f32) -> TableMetrics {
    let available = if available_inner_width.is_finite() && available_inner_width > 0.0 {
        available_inner_width
    } else {
        0.0
    };
    let reserved = TABLE_VSCROLL_RESERVE;
    let gaps = column_gaps();
    let usable = (available - reserved - gaps).max(0.0);
    let fixed = COL_BUSID_WIDTH + COL_VIDPID_WIDTH + COL_ACTION_WIDTH;
    let leftover = (usable - fixed).max(0.0);
    let (device, state) = split_flex_columns(leftover);
    TableMetrics {
        available,
        reserved,
        widths: [COL_BUSID_WIDTH, COL_VIDPID_WIDTH, device, state, COL_ACTION_WIDTH],
    }
}

fn split_flex_columns(leftover: f32) -> (f32, f32) {
    if leftover <= 0.0 {
        return (0.0, 0.0);
    }
    if leftover + 0.5 >= COL_DEVICE_MIN + COL_STATE_MIN {
        let device = (leftover * COL_DEVICE_LEFTOVER_SHARE)
            .clamp(COL_DEVICE_MIN, leftover - COL_STATE_MIN);
        (device, leftover - device)
    } else if leftover + 0.5 >= COL_STATE_MIN + 48.0 {
        // Keep the status typography floor; the device name ellipsizes.
        (leftover - COL_STATE_MIN, COL_STATE_MIN)
    } else {
        let device = (leftover * COL_DEVICE_LEFTOVER_SHARE).clamp(leftover * 0.25, leftover * 0.75);
        (device, leftover - device)
    }
}

/// Scrollbar reserved only when the single toolbar row overflows.
pub const TOOLBAR_HSCROLL_RESERVE: f32 = 14.0;

/// Labels and combo options for a content-sized toolbar (zh or en).
pub struct ToolbarLabels<'a> {
    pub language: &'a str,
    pub lang_options: &'a [&'a str],
    pub theme: &'a str,
    pub theme_options: &'a [&'a str],
    pub refresh: &'a str,
    pub force_bind: &'a str,
    pub filters: &'a str,
    pub settings: &'a str,
}


pub fn text_width(ui: &egui::Ui, text: &str) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let font = egui::TextStyle::Body.resolve(ui.style());
    ui.painter().layout_no_wrap(text.to_owned(), font, egui::Color32::WHITE).size().x
}

fn labeled(text: &str) -> String {
    format!("{text}:")
}

pub fn combo_width(ui: &egui::Ui, options: &[&str]) -> f32 {
    let text = options.iter().copied().map(|s| text_width(ui, s)).fold(0.0, f32::max);
    let pad = ui.spacing().button_padding.x * 2.0;
    let arrow = ui.spacing().icon_width + ui.spacing().icon_spacing;
    (text + pad + arrow).max(80.0)
}

pub fn button_width(ui: &egui::Ui, text: &str) -> f32 {
    let pad = ui.spacing().button_padding.x * 2.0;
    (text_width(ui, text) + pad).max(ui.spacing().interact_size.y)
}

pub fn checkbox_width(ui: &egui::Ui, text: &str) -> f32 {
    ui.spacing().icon_width + ui.spacing().icon_spacing + text_width(ui, text)
}

/// Natural width of the six groups plus 8 px gaps. Does not stretch to the window.
pub fn toolbar_content_width(ui: &egui::Ui, labels: &ToolbarLabels<'_>) -> f32 {
    let lang = text_width(ui, &labeled(labels.language)) + TOOLBAR_INNER_GAP + combo_width(ui, labels.lang_options);
    let theme = text_width(ui, &labeled(labels.theme)) + TOOLBAR_INNER_GAP + combo_width(ui, labels.theme_options);
    let refresh = button_width(ui, labels.refresh);
    let force = checkbox_width(ui, labels.force_bind);
    let filters = button_width(ui, labels.filters);
    let settings = button_width(ui, labels.settings);
    lang + theme + refresh + force + filters + settings + TOOLBAR_GROUP_GAP * 5.0
}

pub fn toolbar_needs_horizontal_scroll(available_width: f32, content_width: f32) -> bool {
    content_width > available_width + 0.5
}

fn sanitize_scale(native_pixels_per_point: f32) -> f32 {
    if native_pixels_per_point.is_finite() && native_pixels_per_point > 0.0 {
        native_pixels_per_point
    } else {
        1.0
    }
}

/// Logical client size that reproduces [`DECORATED_WINDOW_SIZE`] at `scale`, minus the frame.
pub fn decorated_client_size(native_pixels_per_point: f32, frame_points: egui::Vec2) -> egui::Vec2 {
    let scale = sanitize_scale(native_pixels_per_point);
    egui::vec2(DECORATED_WINDOW_SIZE[0], DECORATED_WINDOW_SIZE[1]) / scale
        - frame_points.max(egui::Vec2::ZERO)
}

/// Match an approximately 1024×760 physical decorated reference window.
/// Neither axis is clamped up to a 1.0-scale minimum (that would exceed the physical
/// target at 150% DPI).
pub fn initial_client_size(native_pixels_per_point: f32, frame_points: egui::Vec2) -> egui::Vec2 {
    decorated_client_size(native_pixels_per_point, frame_points).max(egui::Vec2::ZERO)
}

/// Native minimum inner size at this DPI, equal to the default size on both axes.
/// The window can grow, but cannot shrink below the default.
pub fn min_client_size(native_pixels_per_point: f32, frame_points: egui::Vec2) -> egui::Vec2 {
    initial_client_size(native_pixels_per_point, frame_points)
}

pub fn content_bottom_margin() -> f32 {
    BOTTOM_MARGIN
}

pub fn central_content_frame(style: &egui::Style) -> egui::Frame {
    let margin = egui::Margin { left: 8, right: 8, top: 8, bottom: content_bottom_margin() as i8 };
    egui::Frame::central_panel(style).inner_margin(margin)
}

/// Heading row plus the 4 px gap between the table and the log panel.
pub const LOG_CHROME_RESERVE: f32 = 38.0;
/// Share of the flex band given to the device table. The previous default was 0.5
/// (table and log body split evenly). The log body is now half of that former
/// allocation, so the table takes 0.75 and the extra height does not become a gap.
pub const TABLE_FLEX_SHARE: f32 = 0.75;

pub fn table_height(remaining_height: f32) -> f32 {
    // Reserve the log heading and inter-panel gap, then give the table 3/4 of the
    // leftover so the log body is half of its previous default height.
    (remaining_height - LOG_CHROME_RESERVE).max(0.0) * TABLE_FLEX_SHARE
}

/// Reserve exactly this region: oversized scroll content cannot push later panels down.
pub fn fixed_region<R>(ui: &mut egui::Ui, size: egui::Vec2, contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let (rect, _) = ui.allocate_exact_size(size.max(egui::Vec2::ZERO), egui::Sense::hover());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect)
        .layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    contents(&mut child)
}


/// Cell layout used by the table. Left-to-right so the cross axis is vertical.
pub fn cell_center_layout() -> egui::Layout {
    egui::Layout::left_to_right(egui::Align::Center)
}

fn claim_cell(ui: &mut egui::Ui) -> egui::Rect {
    let cell = ui.max_rect();
    ui.advance_cursor_after_rect(cell);
    cell
}

/// Visual (mesh) bounds of a galley in galley-local coordinates.
pub fn galley_visual_bounds(galley: &egui::Galley) -> egui::Rect {
    if galley.mesh_bounds.width() > 0.5 && galley.mesh_bounds.height() > 0.5 {
        galley.mesh_bounds
    } else {
        galley.rect
    }
}

/// Layout a single truncated line and return (galley, origin such that the *painted*
/// mesh is centered in `cell`). This is the painted visual center, not the widget
/// response rect (which can disagree with glyph ink and caused a ~10px mismatch).
pub fn centered_galley_in_cell(
    ui: &mut egui::Ui,
    cell: egui::Rect,
    text: &str,
    color: egui::Color32,
    strong: bool,
) -> (std::sync::Arc<egui::Galley>, egui::Pos2) {
    let mut rich = egui::RichText::new(text);
    if strong {
        rich = rich.strong();
    }
    rich = rich.color(color);
    let valign = egui::Align::Center;
    let mut job = std::sync::Arc::unwrap_or_clone(egui::WidgetText::from(rich).into_layout_job(
        ui.style(),
        egui::FontSelection::Default,
        valign,
    ));
    job.wrap.max_width = cell.width().max(0.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.halign = egui::Align::LEFT;
    job.justify = false;
    let galley = ui.painter().layout_job(job);
    let visual = galley_visual_bounds(&galley);
    let pos = cell.center() - visual.center().to_vec2();
    (galley, pos)
}

/// Paint a (possibly truncated) label whose *glyph mesh* is centered in the cell
/// on both axes. Hover shows the full text when elided. Does not use a two-pass
/// widget measure that can desync from the drawn galley.
pub fn centered_cell_label(
    ui: &mut egui::Ui,
    text: &str,
    color: Option<egui::Color32>,
    strong: bool,
) -> egui::Response {
    let cell = claim_cell(ui);
    let color = color.unwrap_or_else(|| {
        if strong { ui.visuals().strong_text_color() } else { ui.visuals().text_color() }
    });
    let (galley, pos) = centered_galley_in_cell(ui, cell, text, color, strong);
    let elided = galley.elided;
    let id = ui.scope_id().with("centered_cell_label").with((
        cell.min.x.to_bits(),
        cell.min.y.to_bits(),
        cell.max.x.to_bits(),
        cell.max.y.to_bits(),
    ));
    let mut response = ui.interact(cell, id, egui::Sense::hover());
    if elided {
        response = response.on_hover_text(text.to_owned());
    }
    if ui.is_rect_visible(cell) {
        let painter = ui.painter().with_clip_rect(cell.intersect(ui.clip_rect()));
        painter.galley(pos, galley, color);
    }
    response
}

/// Place an interactive widget group (action buttons) so the group's painted
/// background is centered in the cell. Button height is forced to
/// [`ACTION_BUTTON_HEIGHT`] so one- vs two-button rows stay the same height.
pub fn centered_cell_group<R>(ui: &mut egui::Ui, mut add: impl FnMut(&mut egui::Ui) -> R) -> R {
    let cell = ui.max_rect();
    ui.spacing_mut().interact_size.y = ACTION_BUTTON_HEIGHT;
    ui.spacing_mut().button_padding = egui::vec2(6.0, 3.0);
    ui.spacing_mut().item_spacing.x = 4.0;
    let size = ui
        .scope_builder(
            egui::UiBuilder::new()
                .max_rect(cell)
                .layout(egui::Layout::left_to_right(egui::Align::Center))
                .sizing_pass()
                .invisible(),
            |ui| {
                ui.spacing_mut().interact_size.y = ACTION_BUTTON_HEIGHT;
                ui.spacing_mut().button_padding = egui::vec2(6.0, 3.0);
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.horizontal(|ui| add(ui)).response.rect.size()
            },
        )
        .inner;
    let size = if size.x.is_finite() && size.y.is_finite() && size.x > 0.0 && size.y > 0.0 {
        egui::vec2(size.x, ACTION_BUTTON_HEIGHT.max(size.y))
    } else {
        egui::vec2(ACTION_PAIR_MIN_WIDTH, ACTION_BUTTON_HEIGHT)
    };
    let dest = egui::Align2::CENTER_CENTER.anchor_size(cell.center(), size);
    let ir = ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(dest)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.spacing_mut().interact_size.y = ACTION_BUTTON_HEIGHT;
            ui.spacing_mut().button_padding = egui::vec2(6.0, 3.0);
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.horizontal(|ui| add(ui)).inner
        },
    );
    ui.advance_cursor_after_rect(cell);
    ir.inner
}

/// Horizontal scroll only when content is wider than the viewport.
pub fn with_hscroll_if<R>(
    ui: &mut egui::Ui,
    enabled: bool,
    id: &str,
    min_width: f32,
    max_height: f32,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    if enabled {
        egui::ScrollArea::horizontal()
            .id_salt(id)
            .auto_shrink([false, false])
            .min_scrolled_height(0.0)
            .max_height(max_height.max(0.0))
            .show(ui, |ui| {
                ui.set_min_width(min_width);
                contents(ui)
            })
            .inner
    } else {
        contents(ui)
    }
}

/// Horizontal scroll only when the compact/fixed plus minimum flexible columns overflow.
pub fn with_optional_hscroll<R>(
    ui: &mut egui::Ui,
    metrics: TableMetrics,
    id: &str,
    max_height: f32,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    with_hscroll_if(ui, metrics.needs_horizontal_scroll(), id, metrics.table_width(), max_height, contents)
}

/// Always one compact toolbar row: natural control widths, 8 px group gaps, no equal cells.
/// Horizontal scroll only when the content is wider than the viewport.
pub fn compact_toolbar<R>(
    ui: &mut egui::Ui,
    content_width: f32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let available = ui.available_width();
    let scroll = toolbar_needs_horizontal_scroll(available, content_width);
    let height = if scroll {
        TOOLBAR_ROW_HEIGHT + TOOLBAR_HSCROLL_RESERVE
    } else {
        TOOLBAR_ROW_HEIGHT
    };
    with_hscroll_if(ui, scroll, "toolbar_hscroll", content_width, height, |ui| {
        if scroll {
            ui.set_min_width(content_width);
        }
        ui.allocate_ui_with_layout(
            egui::vec2(content_width, TOOLBAR_ROW_HEIGHT),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_height(TOOLBAR_ROW_HEIGHT);
                ui.spacing_mut().item_spacing.x = TOOLBAR_GROUP_GAP;
                ui.spacing_mut().item_spacing.y = 0.0;
                add(ui)
            },
        ).inner
    })
}
