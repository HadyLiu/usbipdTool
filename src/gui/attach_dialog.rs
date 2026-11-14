//! 附加设备到 WSL 的对话框。

use super::theme;
use eframe::egui::{self, RichText};
use usbipd_tool::core::actions::attach_args;
use usbipd_tool::core::i18n::I18n;
use usbipd_tool::core::models::{UsbDevice, WslDistro};
use usbipd_tool::core::usbipd::display_command;

pub struct AttachDialog {
    pub busid: String,
    pub description: String,
    pub use_default: bool,
    /// 可编辑的发行版名称（下拉选择或手动输入）
    pub distro: String,
    pub auto_attach: bool,
    pub host_ip: String,
}

pub enum AttachOutcome {
    Accept,
    Cancel,
}

impl AttachDialog {
    pub fn new(dev: &UsbDevice, distros: &[WslDistro], last_distro: &str, last_host_ip: &str, auto_default: bool) -> Self {
        let distro = if !last_distro.trim().is_empty() {
            last_distro.to_string()
        } else {
            distros
                .iter()
                .find(|d| d.is_default)
                .or(distros.first())
                .map(|d| d.name.clone())
                .unwrap_or_default()
        };
        Self {
            busid: dev.busid.clone(),
            description: dev.description.clone(),
            use_default: true,
            distro,
            auto_attach: auto_default,
            host_ip: last_host_ip.to_string(),
        }
    }

    /// 选定的发行版；勾选「使用默认发行版」时为 None。
    pub fn distribution(&self) -> Option<String> {
        if self.use_default {
            return None;
        }
        Some(self.distro.trim().to_string()).filter(|d| !d.is_empty())
    }

    pub fn host_ip_value(&self) -> String {
        self.host_ip.trim().to_string()
    }

    pub fn args(&self) -> Vec<String> {
        let ip = self.host_ip_value();
        attach_args(
            &self.busid,
            self.distribution().as_deref(),
            Some(ip.as_str()).filter(|s| !s.is_empty()),
            self.auto_attach,
        )
    }

    pub fn show(&mut self, ui: &mut egui::Ui, tr: &I18n, distros: &[WslDistro], dark: bool) -> Option<AttachOutcome> {
        let mut outcome = None;
        ui.set_width(520.0);
        ui.heading(tr.tr("attach_dialog.title"));
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(tr.trf("attach_dialog.info", &[("busid", &self.busid), ("desc", &self.description)]))
                .wrap(),
        );
        ui.add_space(6.0);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.strong(tr.tr("attach_dialog.group_wsl"));
            egui::Grid::new("attach_form").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                ui.label("");
                ui.checkbox(&mut self.use_default, tr.tr("attach_dialog.use_default"));
                ui.end_row();

                ui.label(tr.tr("attach_dialog.distro"));
                ui.add_enabled_ui(!self.use_default, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.distro).desired_width(220.0));
                        let mark = tr.tr("attach_dialog.default_mark");
                        egui::ComboBox::from_id_salt("distro_combo")
                            .selected_text("")
                            .width(24.0)
                            .show_ui(ui, |ui| {
                                for d in distros {
                                    let label = if d.is_default {
                                        format!("{}  ({mark})", d.name)
                                    } else {
                                        d.name.clone()
                                    };
                                    ui.selectable_value(&mut self.distro, d.name.clone(), label);
                                }
                            });
                    });
                });
                ui.end_row();

                ui.label("");
                ui.checkbox(&mut self.auto_attach, tr.tr("attach_dialog.auto_attach"));
                ui.end_row();

                ui.label(tr.tr("attach_dialog.host_ip"));
                ui.add(
                    egui::TextEdit::singleline(&mut self.host_ip)
                        .hint_text(tr.tr("attach_dialog.host_ip_placeholder"))
                        .desired_width(300.0),
                );
                ui.end_row();
            });
        });

        ui.add_space(6.0);
        let preview = tr.trf("attach_dialog.preview", &[("cmd", &display_command(&self.args()))]);
        ui.add(egui::Label::new(RichText::new(preview).color(theme::preview_color(dark))).wrap());
        ui.add_space(8.0);
        super::dialog_layout::buttons(ui, |ui| {
            if ui.button(tr.tr("common.cancel")).clicked() {
                outcome = Some(AttachOutcome::Cancel);
            }
            if ui.button(RichText::new(tr.tr("common.attach")).strong()).clicked() {
                outcome = Some(AttachOutcome::Accept);
            }
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            outcome = Some(AttachOutcome::Cancel);
        }
        outcome
    }
}
