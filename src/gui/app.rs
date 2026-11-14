//! 主窗口。布局与原版一致：信息行 → 工具条 → 设备表格 → 日志面板 → 状态栏。

use super::attach_dialog::{AttachDialog, AttachOutcome};
use super::hotplug::UsbWatcher;
use super::theme::{self, BtnKind};
use super::worker::{AutoAttach, Bus, Msg, OpKind};
use eframe::egui::{self, Align, Color32, Layout, RichText, Vec2};
use egui_extras::{Column, TableBuilder};
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, channel};
use usbipd_tool::core::actions;
use usbipd_tool::core::config::Config;
use usbipd_tool::core::i18n::{self, I18n};
use usbipd_tool::core::models::{UsbDevice, WslDistro};
use usbipd_tool::core::usbipd::{self, CommandResult, display_command};

const MAX_LOG_LINES: usize = 3000;

fn window_title(tr: &I18n) -> String {
    format!("{}  v{}", tr.tr("app.title"), usbipd_tool::VERSION)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Ready,
    Scanning,
    NoDevices,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScanKind {
    Manual,
    Hardware,
    AfterOp,
}

enum Confirm {
    Unbind(String),
    Detach(String),
    ForceBind(String),
}

enum Dialog {
    /// usbipd 未安装：确定后退出
    Fatal(String),
    Warn(String),
    Confirm(String, Confirm),
    Attach(AttachDialog),
}

enum UiAction {
    Bind(String),
    Attach(String),
    Unbind(String),
    Detach(String),
}

pub struct UsbipdApp {
    cfg: Config,
    tr: I18n,
    devices: Vec<UsbDevice>,
    distros: Vec<WslDistro>,
    distros_loaded: bool,
    distros_loading: bool,
    pending_attach: Option<String>,
    busy: bool,
    status: Status,
    version: Option<String>,
    is_admin: bool,
    dark: bool,
    force_bind: bool,
    log: VecDeque<String>,
    log_text: String,
    dialogs: VecDeque<Dialog>,
    rx: Receiver<Msg>,
    bus: Bus,
    auto: AutoAttach,
    usbipd_missing: bool,
    /// 当前正在进行的扫描来源（仅扫描期间有值）
    scan_kind: Option<ScanKind>,
    /// 忙于扫描或 bind/unbind/attach/detach 时，插拔事件排队，结束后再扫一次
    pending_hw_refresh: bool,
    /// 用户操作引起的设备节点变化也会触发 WM_DEVICECHANGE，短时间内不写「硬件变化」日志
    suppress_hw_log_until: Option<std::time::Instant>,
    _hotplug: Option<UsbWatcher>,
}

impl UsbipdApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        super::fonts::install(&ctx);

        let cfg = Config::load();
        let lang = {
            let l = cfg.language();
            if l.is_empty() { i18n::detect_system_lang().to_string() } else { l }
        };
        let tr = I18n::new(&lang);
        let dark = theme::effective_dark(&cfg.theme(), &ctx);
        theme::apply(&ctx, dark);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title(&tr)));

        let (tx, rx) = channel();
        let bus = Bus { tx, ctx };
        let usbipd_missing = usbipd::find_usbipd().is_none();

        let mut app = Self {
            force_bind: cfg.force_bind_default(),
            cfg,
            tr,
            devices: Vec::new(),
            distros: Vec::new(),
            distros_loaded: false,
            distros_loading: false,
            pending_attach: None,
            busy: false,
            status: Status::Ready,
            version: None,
            is_admin: usbipd::is_admin(),
            dark,
            log: VecDeque::new(),
            log_text: String::new(),
            dialogs: VecDeque::new(),
            rx,
            bus,
            auto: AutoAttach::default(),
            usbipd_missing,
            scan_kind: None,
            pending_hw_refresh: false,
            suppress_hw_log_until: None,
            _hotplug: None,
        };

        if app.usbipd_missing {
            let t = app.tr.tr("error.usbipd_not_found");
            app.dialogs.push_back(Dialog::Fatal(t));
        } else {
            app.bus.spawn(|| Msg::Version(usbipd::get_version()));
            app.on_refresh();
            app.warm_distros();
            let bus = app.bus.clone();
            app._hotplug = UsbWatcher::start(move || bus.send(Msg::HardwareChanged));
        }
        app
    }

    // ------------------------------------------------------------ 日志
    fn log(&mut self, text: &str) {
        let ts = chrono::Local::now().format("%H:%M:%S");
        self.log.push_back(format!("[{ts}] {text}"));
        while self.log.len() > MAX_LOG_LINES {
            self.log.pop_front();
        }
        self.log_text = self.log.iter().map(String::as_str).collect::<Vec<_>>().join("\n");
    }

    fn log_output(&mut self, res: &CommandResult) {
        for line in res.stdout.lines().filter(|l| !l.trim().is_empty()) {
            self.log(line);
        }
        for line in res.stderr.lines().filter(|l| !l.trim().is_empty()) {
            self.log(&format!("[stderr] {line}"));
        }
    }

    fn warn(&mut self, text: String) {
        self.dialogs.push_back(Dialog::Warn(text));
    }

    fn cmd_failed(&mut self, res: &CommandResult) {
        let code = res.returncode.to_string();
        let err = res.error_text();
        let t = self.tr.trf("msg.cmd_failed", &[("code", &code), ("err", &err)]);
        self.warn(t);
    }

    // ------------------------------------------------------------ 扫描
    fn on_refresh(&mut self) {
        self.start_scan(ScanKind::Manual);
    }

    fn request_hw_refresh(&mut self) {
        if self.busy {
            self.pending_hw_refresh = true;
        } else {
            self.start_scan(ScanKind::Hardware);
        }
    }

    fn start_scan(&mut self, kind: ScanKind) {
        if self.busy {
            if kind == ScanKind::Hardware {
                self.pending_hw_refresh = true;
            }
            return;
        }
        self.busy = true;
        self.scan_kind = Some(kind);
        self.status = Status::Scanning;
        self.bus.spawn(|| Msg::ScanDone(actions::scan_devices()));
    }

    fn suppress_hw_log(&mut self) {
        self.suppress_hw_log_until = Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
    }

    /// 操作或扫描结束后，若有排队的硬件刷新则立刻执行。
    fn maybe_run_pending_hw_refresh(&mut self) {
        if !self.busy && self.pending_hw_refresh {
            self.pending_hw_refresh = false;
            self.start_scan(ScanKind::Hardware);
        }
    }

    fn devices_signature(devs: &[UsbDevice]) -> Vec<(String, String, String, u8, Option<String>)> {
        let mut v: Vec<_> = devs
            .iter()
            .map(|d| (d.busid.clone(), d.vidpid(), d.description.clone(), d.state as u8, d.client_ip.clone()))
            .collect();
        v.sort();
        v
    }

    fn warm_distros(&mut self) {
        if self.distros_loading {
            return;
        }
        self.distros_loading = true;
        self.bus.spawn(|| Msg::Distros(actions::list_wsl_distros()));
    }

    fn run_op(&mut self, kind: OpKind, busid: &str, args: Vec<String>) {
        self.log(&format!("> {}", display_command(&args)));
        self.busy = true;
        let busid = busid.to_string();
        self.bus.spawn(move || {
            let res = usbipd::run_usbipd(&args, 60);
            Msg::OpDone { kind, busid, res }
        });
    }

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Version(v) => self.version = v,
                Msg::ScanDone(r) => {
                    let kind = self.scan_kind.take();
                    let before = Self::devices_signature(&self.devices);
                    self.busy = false;
                    match r {
                        Ok((devs, warn)) => {
                            self.devices = devs;
                            if let Some(w) = warn {
                                self.log(&format!("[warn] {w}"));
                            }
                        }
                        Err(e) => {
                            self.devices.clear();
                            let t = self.tr.trf("error.scan_failed", &[("msg", &e)]);
                            self.log(&t);
                        }
                    }
                    self.status = if self.devices.is_empty() { Status::NoDevices } else { Status::Ready };
                    if kind == Some(ScanKind::Hardware) {
                        let changed = before != Self::devices_signature(&self.devices);
                        let suppressed = self
                            .suppress_hw_log_until
                            .is_some_and(|t| std::time::Instant::now() < t);
                        if changed && !suppressed {
                            let t = self.tr.tr("msg.hw_refresh");
                            self.log(&t);
                        }
                    }
                    self.maybe_run_pending_hw_refresh();
                }
                Msg::Distros(d) => {
                    self.distros = d;
                    self.distros_loaded = true;
                    self.distros_loading = false;
                    if let Some(b) = self.pending_attach.take() {
                        self.open_attach_dialog(&b);
                    }
                }
                Msg::OpDone { kind, busid, res } => {
                    self.busy = false;
                    self.after_op(kind, &busid, res);
                    self.maybe_run_pending_hw_refresh();
                }
                Msg::Log(line) => self.log(&line),
                Msg::Rescan => self.on_refresh(),
                Msg::HardwareChanged => self.request_hw_refresh(),
                Msg::AutoAttachStarted { busid } => {
                    self.busy = false;
                    self.suppress_hw_log();
                    let t = self.tr.trf("msg.attach_ok", &[("busid", &busid)]);
                    self.log(&format!("{t} (--auto-attach)"));
                    let bus = self.bus.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        bus.send(Msg::Rescan);
                    });
                    self.maybe_run_pending_hw_refresh();
                }
                Msg::AutoAttachExited { busid, code, killed } => {
                    let code_s = code.map(|c| c.to_string()).unwrap_or_else(|| "-".into());
                    self.log(&format!("[auto-attach] {busid} exited (code {code_s})"));
                    if !killed && code != Some(0) {
                        let res = CommandResult::new(code.unwrap_or(-1), "", "usbipd attach --auto-attach exited");
                        self.cmd_failed(&res);
                    }
                    self.suppress_hw_log();
                    self.start_scan(ScanKind::AfterOp);
                    self.maybe_run_pending_hw_refresh();
                }
            }
        }
    }

    fn after_op(&mut self, kind: OpKind, busid: &str, res: CommandResult) {
        self.log_output(&res);
        let ok_key = match kind {
            OpKind::Bind { .. } => "msg.bind_ok",
            OpKind::Unbind => "msg.unbind_ok",
            OpKind::Attach => "msg.attach_ok",
            OpKind::Detach => "msg.detach_ok",
        };
        if res.ok() {
            let t = self.tr.trf(ok_key, &[("busid", busid)]);
            self.log(&t);
            self.suppress_hw_log();
            self.start_scan(ScanKind::AfterOp);
            return;
        }
        if let OpKind::Bind { force: false } = kind {
            if actions::bind_needs_force_hint(&res.error_text()) {
                let t = self.tr.tr("msg.bind_force_hint");
                self.dialogs.push_back(Dialog::Confirm(t, Confirm::ForceBind(busid.to_string())));
                return;
            }
        }
        self.cmd_failed(&res);
    }

    // ------------------------------------------------------------ 操作
    fn do_bind(&mut self, busid: &str, force: bool) {
        self.run_op(OpKind::Bind { force }, busid, actions::bind_args(busid, force));
    }

    fn open_attach_dialog(&mut self, busid: &str) {
        let Some(dev) = self.devices.iter().find(|d| d.busid == busid) else { return };
        let dlg = AttachDialog::new(
            dev,
            &self.distros,
            &self.cfg.last_wsl_distro(),
            &self.cfg.last_host_ip(),
            self.cfg.auto_attach_default(),
        );
        self.dialogs.push_back(Dialog::Attach(dlg));
    }

    fn on_attach(&mut self, busid: &str) {
        if self.distros_loaded {
            self.open_attach_dialog(busid);
        } else {
            self.pending_attach = Some(busid.to_string());
            self.warm_distros();
        }
    }

    fn start_attach(&mut self, dlg: &AttachDialog) {
        let distro = dlg.distribution();
        let host_ip = dlg.host_ip_value();
        self.cfg.set_str("last_wsl_distro", distro.clone().unwrap_or_default());
        self.cfg.set_str("last_host_ip", host_ip.clone());
        self.cfg.set_bool("auto_attach_default", dlg.auto_attach);
        self.cfg.save();

        let args = dlg.args();
        if dlg.auto_attach {
            self.log(&format!("> {}", display_command(&args)));
            self.busy = true;
            if let Err(res) = self.auto.start(&dlg.busid, args, self.bus.clone()) {
                self.busy = false;
                self.log_output(&res);
                self.cmd_failed(&res);
            }
        } else {
            self.run_op(OpKind::Attach, &dlg.busid, args);
        }
    }

    fn handle_ui_action(&mut self, a: UiAction) {
        if self.busy {
            return;
        }
        match a {
            UiAction::Bind(b) => {
                let f = self.force_bind;
                self.do_bind(&b, f);
            }
            UiAction::Attach(b) => self.on_attach(&b),
            UiAction::Unbind(b) => {
                let t = self.tr.trf("msg.unbind_confirm", &[("busid", &b)]);
                self.dialogs.push_back(Dialog::Confirm(t, Confirm::Unbind(b)));
            }
            UiAction::Detach(b) => {
                let t = self.tr.trf("msg.detach_confirm", &[("busid", &b)]);
                self.dialogs.push_back(Dialog::Confirm(t, Confirm::Detach(b)));
            }
        }
    }

    fn handle_confirm(&mut self, c: Confirm) {
        match c {
            Confirm::Unbind(b) => {
                if self.auto.stop(&b) {
                    self.log(&format!("[auto-attach] stopped for {b}"));
                }
                self.run_op(OpKind::Unbind, &b, actions::unbind_args(&b));
            }
            Confirm::Detach(b) => {
                // 先终止 --auto-attach 常驻进程，否则它会立刻重新附加
                if self.auto.stop(&b) {
                    self.log(&format!("[auto-attach] stopped for {b}"));
                }
                self.run_op(OpKind::Detach, &b, actions::detach_args(&b));
            }
            Confirm::ForceBind(b) => self.do_bind(&b, true),
        }
    }

    // ------------------------------------------------------------ 设置
    fn set_language(&mut self, lang: &str) {
        if lang == self.tr.lang() {
            return;
        }
        self.tr = I18n::new(lang);
        self.cfg.set_str("language", lang);
        self.cfg.save();
        self.bus.ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title(&self.tr)));
    }

    fn set_theme(&mut self, pref: &str) {
        self.cfg.set_str("theme", pref);
        self.cfg.save();
        self.dark = theme::effective_dark(pref, &self.bus.ctx);
        theme::apply(&self.bus.ctx, self.dark);
    }

    // ------------------------------------------------------------ 绘制
    fn info_text(&self) -> String {
        let head = match &self.version {
            Some(v) => self.tr.trf("status.usbipd_ready", &[("version", v)]),
            None => self.tr.tr("status.usbipd_detected"),
        };
        let admin = self.tr.tr(if self.is_admin { "status.admin_yes" } else { "status.admin_no" });
        format!("{head}   |   {admin}   |   UsbipdTool v{}", usbipd_tool::VERSION)
    }

    fn status_text(&self) -> String {
        self.tr.tr(match self.status {
            Status::Ready => "common.ready",
            Status::Scanning => "common.scanning",
            Status::NoDevices => "status.no_devices",
        })
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.label(format!("{}:", self.tr.tr("common.language")));
            let mut lang = self.tr.lang().to_string();
            let lang_label = if lang == "zh_CN" { "中文" } else { "English" };
            egui::ComboBox::from_id_salt("lang").selected_text(lang_label).show_ui(ui, |ui| {
                ui.selectable_value(&mut lang, "zh_CN".to_string(), "中文");
                ui.selectable_value(&mut lang, "en_US".to_string(), "English");
            });
            if lang != self.tr.lang() {
                self.set_language(&lang);
            }

            ui.label(format!("{}:", self.tr.tr("common.theme")));
            let mut pref = self.cfg.theme();
            let names = [
                ("", self.tr.tr("theme.system")),
                ("light", self.tr.tr("theme.light")),
                ("dark", self.tr.tr("theme.dark")),
            ];
            let cur = names.iter().find(|(k, _)| *k == pref).map(|(_, n)| n.clone()).unwrap_or_else(|| names[0].1.clone());
            egui::ComboBox::from_id_salt("theme").selected_text(cur).show_ui(ui, |ui| {
                for (k, n) in &names {
                    ui.selectable_value(&mut pref, k.to_string(), n);
                }
            });
            if pref != self.cfg.theme() {
                self.set_theme(&pref);
            }

            if ui.add_enabled(!self.busy, egui::Button::new(self.tr.tr("common.refresh"))).clicked() {
                self.on_refresh();
            }

            let resp = ui
                .checkbox(&mut self.force_bind, self.tr.tr("common.force_bind"))
                .on_hover_text(self.tr.tr("common.force_bind_tip"));
            if resp.changed() {
                self.cfg.set_bool("force_bind_default", self.force_bind);
                self.cfg.save();
            }
        });
    }

    fn device_table(&self, ui: &mut egui::Ui, height: f32, actions_out: &mut Vec<UiAction>) {
        let tr = &self.tr;
        let dark = self.dark;
        let busy = self.busy;
        let border = if dark { Color32::from_rgb(0x3a, 0x3c, 0x44) } else { Color32::from_rgb(0xd9, 0xdc, 0xe1) };
        ui.allocate_ui(Vec2::new(ui.available_width(), height), |ui| {
            egui::Frame::new()
                .fill(ui.visuals().extreme_bg_color)
                .stroke(egui::Stroke::new(1.0, border))
                .corner_radius(4.0)
                .inner_margin(4.0)
                .show(ui, |ui| {
                    ui.set_min_size(Vec2::new(ui.available_width(), height - 10.0));
                    let headers = ["col.busid", "col.vidpid", "col.device", "col.state", "col.action"];
                    TableBuilder::new(ui)
                        .id_salt("devices")
                        .striped(true)
                        .resizable(false)
                        .auto_shrink([false, false])
                        .cell_layout(Layout::left_to_right(Align::Center))
                        .column(Column::auto().at_least(56.0))
                        .column(Column::auto().at_least(80.0))
                        .column(Column::remainder().at_least(140.0).clip(true))
                        .column(Column::auto().at_least(80.0))
                        .column(Column::exact(190.0))
                        .min_scrolled_height(0.0)
                        .max_scroll_height((height - 46.0).max(40.0))
                        .header(26.0, |mut h| {
                            for k in headers {
                                h.col(|ui| {
                                    ui.strong(tr.tr(k));
                                });
                            }
                        })
                        .body(|mut body| {
                            for dev in &self.devices {
                                body.row(30.0, |mut row| {
                                    row.col(|ui| {
                                        ui.label(&dev.busid);
                                    });
                                    row.col(|ui| {
                                        let vp = dev.vidpid();
                                        ui.label(if vp.is_empty() { "-".to_string() } else { vp });
                                    });
                                    row.col(|ui| {
                                        let desc = if !dev.description.is_empty() {
                                            dev.description.clone()
                                        } else {
                                            dev.instance_id.clone().unwrap_or_else(|| "-".into())
                                        };
                                        ui.add(egui::Label::new(&desc).truncate()).on_hover_text(&desc);
                                    });
                                    row.col(|ui| {
                                        let mut s = tr.tr(dev.state.i18n_key());
                                        if dev.is_attached() {
                                            if let Some(ip) = &dev.client_ip {
                                                s.push_str(&format!(" ({ip})"));
                                            }
                                        }
                                        if self.auto.is_running(&dev.busid) {
                                            s.push_str(" [auto]");
                                        }
                                        ui.label(RichText::new(s).color(theme::state_color(dev.state, dark)));
                                    });
                                    row.col(|ui| {
                                        let btn = |ui: &mut egui::Ui, key: &str, kind: BtnKind| -> bool {
                                            let b = egui::Button::new(
                                                RichText::new(tr.tr(key)).color(theme::button_color(kind, dark)),
                                            );
                                            ui.add_enabled(!busy, b)
                                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                                .clicked()
                                        };
                                        let id = dev.busid.clone();
                                        if dev.is_not_shared() {
                                            if btn(ui, "btn.bind", BtnKind::Bind) {
                                                actions_out.push(UiAction::Bind(id.clone()));
                                            }
                                        } else if dev.is_shared() {
                                            if btn(ui, "btn.attach", BtnKind::Attach) {
                                                actions_out.push(UiAction::Attach(id.clone()));
                                            }
                                            if btn(ui, "btn.unbind", BtnKind::Unbind) {
                                                actions_out.push(UiAction::Unbind(id.clone()));
                                            }
                                        } else if dev.is_attached() {
                                            if btn(ui, "btn.detach", BtnKind::Detach) {
                                                actions_out.push(UiAction::Detach(id.clone()));
                                            }
                                            if btn(ui, "btn.unbind", BtnKind::Unbind) {
                                                actions_out.push(UiAction::Unbind(id.clone()));
                                            }
                                        }
                                    });
                                });
                            }
                        });
                });
        });
    }

    fn log_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong(self.tr.tr("log.title"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(self.tr.tr("common.clear")).clicked() {
                    self.log.clear();
                    self.log_text.clear();
                }
            });
        });
        let dark = self.dark;
        egui::Frame::new()
            .fill(theme::log_bg(dark))
            .stroke(egui::Stroke::new(1.0, if dark { Color32::from_rgb(0x2b, 0x2d, 0x33) } else { Color32::from_rgb(0xd0, 0xd7, 0xde) }))
            .corner_radius(4.0)
            .inner_margin(4.0)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                egui::ScrollArea::vertical()
                    .id_salt("log_scroll")
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.log_text.as_str())
                                .font(egui::TextStyle::Monospace)
                                .text_color(theme::log_fg(dark))
                                .frame(egui::Frame::NONE)
                                .desired_width(f32::INFINITY),
                        );
                    });
            });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(front) = self.dialogs.front_mut() else { return };
        let tr = &self.tr;
        let dark = self.dark;
        let distros = &self.distros;
        // 返回值：Some(true)=确认/接受，Some(false)=取消
        let result: Option<bool> = egui::Modal::new(egui::Id::unique("usbipd_modal"))
            .show(ctx, |ui| match front {
                Dialog::Fatal(t) | Dialog::Warn(t) => {
                    ui.set_max_width(460.0);
                    ui.strong("UsbipdTool");
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(t.as_str()).wrap());
                    ui.add_space(10.0);
                    let mut r = None;
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(tr.tr("common.ok")).clicked() {
                            r = Some(true);
                        }
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)) {
                        r = Some(true);
                    }
                    r
                }
                Dialog::Confirm(t, _) => {
                    ui.set_max_width(460.0);
                    ui.strong("UsbipdTool");
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(t.as_str()).wrap());
                    ui.add_space(10.0);
                    let mut r = None;
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(tr.tr("common.no")).clicked() {
                            r = Some(false);
                        }
                        if ui.button(tr.tr("common.yes")).clicked() {
                            r = Some(true);
                        }
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        r = Some(false);
                    }
                    r
                }
                Dialog::Attach(d) => d.show(ui, tr, distros, dark).map(|o| matches!(o, AttachOutcome::Accept)),
            })
            .inner;

        let Some(accepted) = result else { return };
        let Some(dlg) = self.dialogs.pop_front() else { return };
        match dlg {
            Dialog::Fatal(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Dialog::Warn(_) => {}
            Dialog::Confirm(_, c) => {
                if accepted {
                    self.handle_confirm(c);
                }
            }
            Dialog::Attach(d) => {
                if accepted {
                    self.start_attach(&d);
                }
            }
        }
    }
}

impl eframe::App for UsbipdApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_messages();
        let ctx = ui.ctx().clone();

        if self.usbipd_missing {
            egui::CentralPanel::default_margins().show(ui, |_ui| {});
            self.dialogs(&ctx);
            return;
        }

        if self.busy {
            ctx.set_cursor_icon(egui::CursorIcon::Progress);
        }

        egui::Panel::bottom("status_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.status_text());
                if self.busy {
                    ui.spinner();
                }
            });
        });

        egui::Panel::top("top_bar").show_separator_line(false).show(ui, |ui| {
            ui.add_space(6.0);
            ui.add(egui::Label::new(RichText::new(self.info_text()).strong()).wrap());
            ui.add_space(4.0);
            self.toolbar(ui);
            ui.add_space(4.0);
        });

        let mut acts = Vec::new();
        egui::CentralPanel::default_margins().show(ui, |ui| {
            let avail = ui.available_height();
            let table_h = ((avail - 34.0) / 2.0).max(120.0);
            self.device_table(ui, table_h, &mut acts);
            ui.add_space(4.0);
            self.log_panel(ui);
        });
        for a in acts {
            self.handle_ui_action(a);
        }

        self.dialogs(&ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // --auto-attach 常驻进程没有窗口，退出时一并终止，避免残留
        self.auto.stop_all();
        // 关闭隐藏的 USB 监听窗口
        self._hotplug = None;
    }
}
