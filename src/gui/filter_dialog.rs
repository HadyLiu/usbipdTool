//! Display-filter rules window: edit a draft, Save persists, Cancel discards.
//! Opening or saving rules never issues USB bind/unbind/attach/detach commands.

use eframe::egui::{self, RichText};
use usbipd_tool::core::device_filter::{self, FilterField, FilterRule, RuleError};
use usbipd_tool::core::i18n::I18n;

pub enum FilterOutcome {
    Save,
    Cancel,
}

pub struct FilterDialog {
    draft: Vec<FilterRule>,
    /// Session-only; kept in sync with the main window while this window is open.
    pub show_all: bool,
    error: Option<RuleError>,
    /// Bottom of the laid-out form, in window coordinates. The window hugs this.
    pub content_bottom: f32,
}

impl FilterDialog {
    pub fn from_rules(rules: Vec<FilterRule>, show_all: bool) -> Self {
        Self { draft: rules, show_all, error: None, content_bottom: 0.0 }
    }

    pub fn take_rules(self) -> Vec<FilterRule> {
        self.draft
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        tr: &I18n,
        hidden: usize,
        total: usize,
    ) -> Option<FilterOutcome> {
        let mut outcome = None;
        // Slightly wider than settings so field + value + delete stay on one row.
        let form_w = super::dialog_layout::column_width(
            ui.available_width(),
            super::dialog_layout::FILTERS_FORM_WIDTH,
        );
        super::dialog_layout::show_column(ui, form_w, |ui| {
            super::dialog_layout::apply_form_width(ui, form_w);

            ui.heading(tr.tr("filter.title"));
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(tr.tr("filter.semantics")).small()).wrap());
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label(tr.trf("filter.hidden_count", &[("n", &hidden.to_string())]));
                ui.label(format!("/ {total}"));
                let toggle = if self.show_all {
                    tr.tr("filter.resume")
                } else {
                    tr.tr("filter.show_all")
                };
                if ui.add_enabled(hidden > 0, egui::Button::new(toggle)).clicked() {
                    self.show_all = !self.show_all;
                }
                if self.show_all {
                    ui.label(RichText::new(tr.tr("filter.showing_all")).small());
                }
            });
            ui.add_space(6.0);

            let max_body = super::dialog_layout::FILTERS_BODY_MAX;
            egui::ScrollArea::vertical()
                .id_salt("filter_rules_scroll")
                .max_height(max_body)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    super::dialog_layout::apply_form_width(ui, form_w);
                    self.show_body(ui, tr, form_w);
                });

            ui.add_space(8.0);
            super::dialog_layout::buttons(ui, |ui| {
                if ui.button(RichText::new(tr.tr("common.save")).strong()).clicked() {
                    match device_filter::validate_rules(&self.draft) {
                        Ok(()) => {
                            self.error = None;
                            outcome = Some(FilterOutcome::Save);
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
                if ui.button(tr.tr("common.cancel")).clicked() {
                    outcome = Some(FilterOutcome::Cancel);
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                outcome = Some(FilterOutcome::Cancel);
            }
            self.content_bottom = ui.min_rect().bottom();
        });
        outcome
    }

    fn show_body(&mut self, ui: &mut egui::Ui, tr: &I18n, form_w: f32) {
        let mut remove = None;
        for (i, rule) in self.draft.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut rule.enabled, "");
                let field_label = tr.tr(rule.field.i18n_key());
                egui::ComboBox::from_id_salt(("filter_field", i))
                    .width(132.0)
                    .selected_text(field_label)
                    .show_ui(ui, |ui| {
                        for field in FilterField::ALL {
                            ui.selectable_value(&mut rule.field, field, tr.tr(field.i18n_key()));
                        }
                    });
                let value_w = (form_w - 200.0).clamp(80.0, form_w);
                ui.add(
                    egui::TextEdit::singleline(&mut rule.value)
                        .desired_width(value_w)
                        .hint_text(tr.tr(rule.field.placeholder_key())),
                );
                if ui.button(tr.tr("filter.delete")).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.draft.remove(i);
        }
        ui.add_space(4.0);
        if ui.button(tr.tr("filter.add")).clicked() {
            self.draft.push(FilterRule::default());
        }
        if let Some(err) = self.error {
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(RichText::new(tr.tr(err.i18n_key())).color(ui.visuals().error_fg_color))
                    .wrap(),
            );
        }
    }
}
