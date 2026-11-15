//! 通用附加设置窗口：编辑并保存 WSL 目标偏好，不绑定具体设备，也不执行附加。

use super::theme;
use eframe::egui::{self, RichText};
use usbipd_tool::core::attach_prefs::{self, AttachPrefs, PrefsError};
use usbipd_tool::core::i18n::I18n;
use usbipd_tool::core::models::WslDistro;
use usbipd_tool::core::usbipd::display_command;

pub enum SettingsOutcome {
    Confirm,
    Cancel,
}

pub struct SettingsDialog {
    pub prefs: AttachPrefs,
    error: Option<PrefsError>,
    /// Once the user edits the distro field, later list results must not overwrite it.
    user_changed_distro: bool,
    /// Bottom of the laid-out form, in window coordinates. The window hugs this.
    pub content_bottom: f32,
}

impl SettingsDialog {
    pub fn from_prefs(prefs: AttachPrefs) -> Self {
        Self { prefs, error: None, user_changed_distro: false, content_bottom: 0.0 }
    }

    /// Fill a blank or invalid draft from a finished distro list. Never writes config.
    pub fn apply_loaded_distros(&mut self, distros: &[WslDistro]) {
        if self.user_changed_distro {
            return;
        }
        if let Some(name) = attach_prefs::fill_blank_or_invalid_draft(&self.prefs.distro, distros) {
            self.prefs.distro = name;
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        tr: &I18n,
        distros: &[WslDistro],
        distros_loading: bool,
        distros_error: Option<&str>,
        dark: bool,
    ) -> Option<SettingsOutcome> {
        let mut outcome = None;
        let form_w = super::dialog_layout::column_width(
            ui.available_width(),
            super::dialog_layout::SETTINGS_FORM_WIDTH,
        );
        super::dialog_layout::show_column(ui, form_w, |ui| {
            super::dialog_layout::apply_form_width(ui, form_w);

            // Title stays outside the scroll area so the header remains visible.
            ui.heading(tr.tr("settings.title"));
            ui.add_space(6.0);

            let max_body = super::dialog_layout::SETTINGS_BODY_MAX;
            egui::ScrollArea::vertical()
                .id_salt("settings_body_scroll")
                .max_height(max_body)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    super::dialog_layout::apply_form_width(ui, form_w);
                    self.show_body(ui, tr, distros, distros_loading, distros_error, dark, form_w);
                });

            ui.add_space(8.0);
            // Footer stays outside the scroll area so Confirm/Cancel stay visible.
            super::dialog_layout::buttons(ui, |ui| {
                if ui.button(RichText::new(tr.tr("common.confirm")).strong()).clicked() {
                    match self.prefs.validate() {
                        Ok(()) => {
                            self.error = None;
                            outcome = Some(SettingsOutcome::Confirm);
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
                if ui.button(tr.tr("common.cancel")).clicked() {
                    outcome = Some(SettingsOutcome::Cancel);
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                outcome = Some(SettingsOutcome::Cancel);
            }
            self.content_bottom = ui.min_rect().bottom();
        });
        outcome
    }

    fn show_body(
        &mut self,
        ui: &mut egui::Ui,
        tr: &I18n,
        distros: &[WslDistro],
        distros_loading: bool,
        distros_error: Option<&str>,
        dark: bool,
        form_w: f32,
    ) {

        let default_name = attach_prefs::default_distro_name(distros);
        let use_default_label = match default_name {
            Some(n) => tr.trf("attach_dialog.use_default_named", &[("name", n)]),
            None => tr.tr("attach_dialog.use_default"),
        };

        let field_w = (form_w - 120.0).clamp(80.0, form_w);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_max_width(form_w);
            ui.set_width(ui.available_width().min(form_w));
            ui.strong(tr.tr("attach_dialog.group_wsl"));
            egui::Grid::new("settings_form").num_columns(2).spacing([10.0, 8.0]).max_col_width(form_w).show(ui, |ui| {
                ui.label("");
                ui.checkbox(&mut self.prefs.use_default_distro, use_default_label);
                ui.end_row();

                ui.label(tr.tr("attach_dialog.distro"));
                ui.horizontal(|ui| {
                    if self.prefs.use_default_distro {
                        let mut shown = default_name.unwrap_or("").to_string();
                        ui.add_enabled(
                            false,
                            egui::TextEdit::singleline(&mut shown).desired_width(field_w),
                        );
                    } else {
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.prefs.distro).desired_width(field_w),
                        );
                        if resp.changed() {
                            self.user_changed_distro = true;
                        }
                        let mark = tr.tr("attach_dialog.default_mark");
                        let combo = egui::ComboBox::from_id_salt("settings_distro_combo")
                            .selected_text("")
                            .width(24.0)
                            .show_ui(ui, |ui| {
                                for d in distros {
                                    let label = if d.is_default {
                                        format!("{}  ({mark})", d.name)
                                    } else {
                                        d.name.clone()
                                    };
                                    if ui.selectable_value(&mut self.prefs.distro, d.name.clone(), label).clicked() {
                                        self.user_changed_distro = true;
                                    }
                                }
                            });
                        let _ = combo;
                    }
                    if distros_loading && distros.is_empty() {
                        ui.spinner();
                        ui.add(egui::Label::new(RichText::new(tr.tr("settings.loading_distros")).small()).wrap());
                    } else if distros_loading {
                        ui.spinner();
                    }
                });
                ui.end_row();

                ui.label("");
                ui.checkbox(&mut self.prefs.auto_attach, tr.tr("attach_dialog.auto_attach"));
                ui.end_row();

                ui.label(tr.tr("attach_dialog.host_ip"));
                ui.add(
                    egui::TextEdit::singleline(&mut self.prefs.host_ip)
                        .hint_text(tr.tr("attach_dialog.host_ip_placeholder"))
                        .desired_width(field_w),
                );
                ui.end_row();
            });
        });

        if let Some(msg) = distros_error {
            ui.add_space(4.0);
            let text = tr.trf("settings.distro_load_failed", &[("msg", msg)]);
            ui.add(egui::Label::new(RichText::new(text).color(ui.visuals().error_fg_color)).wrap());
        } else if !distros_loading && distros.is_empty() && default_name.is_none() {
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(tr.tr("settings.no_distros")).small()).wrap());
        }

        ui.add_space(6.0);
        let preview = tr.trf("attach_dialog.preview", &[("cmd", &display_command(&self.prefs.preview_args()))]);
        ui.add(egui::Label::new(RichText::new(preview).color(theme::preview_color(dark))).wrap());

        if let Some(err) = self.error {
            ui.add_space(4.0);
            ui.add(egui::Label::new(RichText::new(tr.tr(err.i18n_key())).color(ui.visuals().error_fg_color)).wrap());
        }

    }
}
