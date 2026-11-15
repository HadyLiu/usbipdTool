//! 主窗口：IOT_Power 风格的页首、紧凑控制栏、设备表格与日志。

use super::filter_dialog::{FilterDialog, FilterOutcome};
use super::settings_dialog::{SettingsDialog, SettingsOutcome};
use usbipd_tool::core::attach_prefs::AttachPrefs;
use super::hotplug::UsbWatcher;
use super::theme::{self, BtnKind};
use super::worker::{AutoAttach, Bus, Msg, OpKind};
use eframe::egui::{self, Align, Layout, RichText, Vec2};
use egui_extras::{Column, TableBuilder};
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Receiver, channel};
use usbipd_tool::core::actions;
use usbipd_tool::core::config::Config;
use usbipd_tool::core::device_filter::{self, FilterRule};
use usbipd_tool::core::device_intent::{self, DesiredState, DeviceIntent, ReconcileDecision, RestoreAction, RestoreBackoff};
use usbipd_tool::core::i18n::{self, I18n};
use usbipd_tool::core::models::{UsbDevice, WslDistro};
use usbipd_tool::core::usbipd::{self, CommandResult, display_command};

const MAX_LOG_LINES: usize = 3000;

fn window_title(tr: &I18n) -> String {
    format!("{}  v{}", tr.tr("app.title"), usbipd_tool::VERSION)
}

fn settings_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("usbipd_settings_window")
}

fn filters_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("usbipd_filters_window")
}

fn focus_viewport(ctx: &egui::Context, id: egui::ViewportId) {
    ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Focus);
}

/// Size and position are applied only on the first frame so later user moves stick.
fn tool_window(
    title: &str,
    inner: [f32; 2],
    min: [f32; 2],
    pos: Option<egui::Pos2>,
    first: bool,
) -> egui::ViewportBuilder {
    let mut builder = egui::ViewportBuilder::default().with_title(title);
    if first {
        builder = builder
            .with_inner_size(inner)
            .with_min_inner_size(min)
            .with_active(true);
        if let Some(pos) = pos {
            builder = builder.with_position(pos);
        }
    }
    builder
}

/// Keep a tool window only as tall as the form that was just laid out.
fn hug_content_height(ui: &egui::Ui, content_bottom: f32, min_h: f32) {
    if !content_bottom.is_finite() || content_bottom < 40.0 {
        return;
    }
    let desired = (content_bottom + super::dialog_layout::CONTENT_BOTTOM_PAD).clamp(min_h, 640.0);
    let Some(rect) = ui.input(|i| i.viewport().inner_rect) else { return };
    if (rect.height() - desired).abs() <= 2.0 {
        return;
    }
    ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(rect.width(), desired)));
}

fn show_tool_window<T>(
    ctx: &egui::Context,
    id: egui::ViewportId,
    builder: egui::ViewportBuilder,
    mut paint: impl FnMut(&mut egui::Ui) -> T,
) -> T {
    ctx.show_viewport_immediate(id, builder, |ui, class| {
        if class == egui::ViewportClass::EmbeddedWindow {
            paint(ui)
        } else {
            egui::CentralPanel::default().show(ui, |ui| paint(ui)).inner
        }
    })
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
    ForceBind(String),
}

enum Dialog {
    /// usbipd 未安装：确定后退出
    Fatal(String),
    Warn(String),
    Confirm(String, Confirm),
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
    distros_error: Option<String>,
    /// Generation so late WSL-list replies cannot overwrite a newer request or a user edit race.
    distro_gen: u64,
    busy: bool,
    status: Status,
    version: Option<String>,
    is_admin: bool,
    dark: bool,
    force_bind: bool,
    logo: egui::TextureHandle,
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
    /// Remembered connection intent, persisted in config. Not inferred from a scan.
    intents: Vec<DeviceIntent>,
    /// Captured at the start of an explicit user op; committed only after success.
    pending_user_intent: Option<(String, DesiredState)>,
    restore_in_flight: Option<RestoreAction>,
    restore_backoff: HashMap<String, RestoreBackoff>,
    /// Display-only hide rules. Raw `devices` is never removed.
    filter_rules: Vec<FilterRule>,
    /// Session-only; temporarily shows hidden devices. Not persisted.
    filter_show_all: bool,
    /// Standalone settings window. Absent means the window is closed.
    settings: Option<SettingsDialog>,
    /// First frame of the settings window applies the initial size and position.
    settings_ready: bool,
    settings_pos: Option<egui::Pos2>,
    /// Standalone filter-rules window. Absent means the window is closed.
    filters: Option<FilterDialog>,
    filters_ready: bool,
    filters_pos: Option<egui::Pos2>,
}

impl UsbipdApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = cc.egui_ctx.clone();
        super::fonts::install(&ctx);

        let logo_image = image::load_from_memory(include_bytes!(concat!(env!("OUT_DIR"), "/icon.png")))
            .expect("generated icon.png").into_rgba8();
        let logo = ctx.load_texture("app-logo",
            egui::ColorImage::from_rgba_unmultiplied([256, 256], logo_image.as_raw()),
            egui::TextureOptions::LINEAR);
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

        let intents = device_intent::load_intents(&cfg);
        let filter_rules = device_filter::load_rules(&cfg);
        let mut app = Self {
            force_bind: cfg.force_bind_default(),
            logo,
            cfg,
            tr,
            devices: Vec::new(),
            distros: Vec::new(),
            distros_loaded: false,
            distros_loading: false,
            distros_error: None,
            distro_gen: 0,
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
            intents,
            pending_user_intent: None,
            restore_in_flight: None,
            restore_backoff: HashMap::new(),
            filter_rules,
            filter_show_all: false,
            settings: None,
            settings_ready: false,
            settings_pos: None,
            filters: None,
            filters_ready: false,
            filters_pos: None,
        };

        // Read-only WSL inventory at startup (list/default name only). Does not set
        // busy, does not run `wsl --update` or change the default distro, and does
        // not wait for or block the USB scan / direct attach path.
        app.warm_distros();
        if app.usbipd_missing {
            let t = app.tr.tr("error.usbipd_not_found");
            app.dialogs.push_back(Dialog::Fatal(t));
        } else {
            app.bus.spawn(|| Msg::Version(usbipd::get_version()));
            app.on_refresh();
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
        self.distro_gen = self.distro_gen.wrapping_add(1);
        let generation = self.distro_gen;
        self.bus.spawn(move || Msg::Distros { generation, result: actions::list_wsl_distros() });
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
                        Ok((mut devs, warn)) => {
                            super::device_order::sort_by_busid_descending(&mut devs);
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
                    self.refresh_intent_identities();
                    self.maybe_restore_intents();
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
                Msg::Distros { generation, result } => {
                    if generation != self.distro_gen {
                        // Stale reply from an earlier list request.
                        continue;
                    }
                    self.distros_loading = false;
                    self.distros_loaded = true;
                    match result {
                        Ok(d) => {
                            self.distros = d;
                            self.distros_error = None;
                        }
                        Err(e) => {
                            self.distros_error = Some(e);
                            // Keep any previously cached list so Settings can still show it.
                        }
                    }
                    if self.distros_error.is_none() {
                        if let Some(dlg) = self.settings.as_mut() {
                            dlg.apply_loaded_distros(&self.distros);
                        }
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
                    if self.restore_in_flight.take().is_some() {
                        self.restore_backoff_success(&busid);
                    } else {
                        self.commit_user_intent(&busid);
                    }
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
        let restore = self.restore_in_flight.take();
        if res.ok() {
            if restore.is_some() {
                self.restore_backoff_success(busid);
            } else {
                self.commit_user_intent(busid);
            }
            let t = self.tr.trf(ok_key, &[("busid", busid)]);
            self.log(&t);
            self.suppress_hw_log();
            self.start_scan(ScanKind::AfterOp);
            return;
        }
        self.pending_user_intent = None;
        if restore.is_some() {
            self.restore_backoff_failure(busid);
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
        if self.restore_in_flight.is_none() {
            self.pending_user_intent = Some((busid.to_string(), DesiredState::Shared));
        }
        self.run_op(OpKind::Bind { force }, busid, actions::bind_args(busid, force));
    }

    fn open_settings(&mut self) {
        if self.settings.is_some() {
            focus_viewport(&self.bus.ctx, settings_viewport_id());
            return;
        }
        let mut dlg = SettingsDialog::from_prefs(AttachPrefs::from_config(&self.cfg));
        if self.distros_loaded && self.distros_error.is_none() {
            dlg.apply_loaded_distros(&self.distros);
        }
        self.settings_pos = self.cascade_pos(0.0);
        self.settings_ready = false;
        self.settings = Some(dlg);
        // Lightweight re-list if idle; cache stays visible, UI never blocks.
        self.warm_distros();
    }

    fn open_filters(&mut self) {
        if self.filters.is_some() {
            focus_viewport(&self.bus.ctx, filters_viewport_id());
            return;
        }
        self.filters_pos = self.cascade_pos(1.0);
        self.filters_ready = false;
        self.filters = Some(FilterDialog::from_rules(
            self.filter_rules.clone(),
            self.filter_show_all,
        ));
    }

    fn cascade_pos(&self, slot: f32) -> Option<egui::Pos2> {
        self.bus.ctx.input(|i| {
            i.viewport().outer_rect.map(|r| r.left_top() + egui::vec2(36.0 + slot * 28.0, 48.0))
        })
    }

    fn on_attach(&mut self, busid: &str) {
        if self.restore_in_flight.is_none() {
            self.pending_user_intent = Some((busid.to_string(), DesiredState::Attached));
        }
        let prefs = AttachPrefs::from_config(&self.cfg);
        match prefs.attach_args(busid) {
            Ok(args) => self.start_attach(busid, args, prefs.auto_attach),
            Err(e) => {
                self.pending_user_intent = None;
                let t = self.tr.tr(e.i18n_key());
                self.warn(t);
            }
        }
    }

    fn start_attach(&mut self, busid: &str, args: Vec<String>, auto_attach: bool) {
        if auto_attach {
            self.log(&format!("> {}", display_command(&args)));
            self.busy = true;
            if let Err(res) = self.auto.start(busid, args, self.bus.clone()) {
                self.busy = false;
                self.log_output(&res);
                self.pending_user_intent = None;
                if self.restore_in_flight.take().is_some() {
                    self.restore_backoff_failure(busid);
                }
                self.cmd_failed(&res);
            }
        } else {
            self.run_op(OpKind::Attach, busid, args);
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
            UiAction::Unbind(b) => self.do_unbind(&b),
            UiAction::Detach(b) => {
                // 先终止 --auto-attach 常驻进程，否则它会立刻重新附加。
                if self.auto.stop(&b) {
                    self.log(&format!("[auto-attach] stopped for {b}"));
                }
                self.pending_user_intent = Some((b.clone(), DesiredState::Shared));
                self.run_op(OpKind::Detach, &b, actions::detach_args(&b));
            }
        }
    }

    fn do_unbind(&mut self, busid: &str) {
        if self.busy { return; }
        if self.auto.stop(busid) {
            self.log(&format!("[auto-attach] stopped for {busid}"));
        }
        if self.restore_in_flight.is_none() {
            self.pending_user_intent = Some((busid.to_string(), DesiredState::Unshared));
        }
        self.run_op(OpKind::Unbind, busid, actions::unbind_args(busid));
    }

    fn handle_confirm(&mut self, c: Confirm) {
        match c {
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

    fn status_text(&self) -> Option<String> {
        match self.status {
            // Idle "就绪" / "Ready" is omitted and does not reserve a footer.
            Status::Ready => None,
            Status::Scanning => Some(self.tr.tr("common.scanning")),
            Status::NoDevices => Some(self.tr.tr("status.no_devices")),
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let language = self.tr.tr("common.language");
        let theme = self.tr.tr("common.theme");
        let refresh = self.tr.tr("common.refresh");
        let force_bind = self.tr.tr("common.force_bind");
        let force_tip = self.tr.tr("common.force_bind_tip");
        let filters = self.tr.tr("common.filters");
        let settings = self.tr.tr("common.settings");
        let t_system = self.tr.tr("theme.system");
        let t_light = self.tr.tr("theme.light");
        let t_dark = self.tr.tr("theme.dark");
        let labels = super::window_layout::ToolbarLabels {
            language: &language,
            lang_options: &["中文", "English"],
            theme: &theme,
            theme_options: &[&t_system, &t_light, &t_dark],
            refresh: &refresh,
            force_bind: &force_bind,
            filters: &filters,
            settings: &settings,
        };
        debug_assert_eq!(super::window_layout::TOOLBAR_SECTIONS, 6);
        let content_w = super::window_layout::toolbar_content_width(ui, &labels);
        let lang_w = super::window_layout::combo_width(ui, labels.lang_options);
        let theme_w = super::window_layout::combo_width(ui, labels.theme_options);
        let inner = super::window_layout::TOOLBAR_INNER_GAP;
        super::window_layout::compact_toolbar(ui, content_w, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = inner;
                ui.label(format!("{language}:"));
                let mut lang = self.tr.lang().to_string();
                let lang_label = if lang == "zh_CN" { "中文" } else { "English" };
                egui::ComboBox::from_id_salt("lang").width(lang_w).selected_text(lang_label).show_ui(ui, |ui| {
                    ui.selectable_value(&mut lang, "zh_CN".to_string(), "中文");
                    ui.selectable_value(&mut lang, "en_US".to_string(), "English");
                });
                if lang != self.tr.lang() {
                    self.set_language(&lang);
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = inner;
                ui.label(format!("{theme}:"));
                let mut pref = self.cfg.theme();
                let names = [("", t_system.as_str()), ("light", t_light.as_str()), ("dark", t_dark.as_str())];
                let cur = names.iter().find(|(k, _)| *k == pref).map(|(_, n)| (*n).to_string()).unwrap_or_else(|| t_system.clone());
                egui::ComboBox::from_id_salt("theme").width(theme_w).selected_text(cur).show_ui(ui, |ui| {
                    for (k, n) in &names {
                        ui.selectable_value(&mut pref, k.to_string(), *n);
                    }
                });
                if pref != self.cfg.theme() {
                    self.set_theme(&pref);
                }
            });
            if ui.add_enabled(!self.busy, egui::Button::new(&refresh)).clicked() {
                self.on_refresh();
            }
            let resp = ui.checkbox(&mut self.force_bind, &force_bind).on_hover_text(&force_tip);
            if resp.changed() {
                self.cfg.set_bool("force_bind_default", self.force_bind);
                self.cfg.save();
            }
            if ui.button(&filters).clicked() {
                self.open_filters();
            }
            if ui.button(&settings).clicked() {
                self.open_settings();
            }
        });
    }

    fn device_table(&self, ui: &mut egui::Ui, height: f32, actions_out: &mut Vec<UiAction>) {
        let tr = &self.tr;
        let dark = self.dark;
        let busy = self.busy;
        let border = theme::border(dark);
        super::window_layout::fixed_region(ui, Vec2::new(ui.available_width(), height), |ui| {
            egui::Frame::new()
                .fill(ui.visuals().extreme_bg_color)
                .stroke(egui::Stroke::new(1.0, border))
                .corner_radius(4.0)
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.set_min_size(Vec2::new(ui.available_width(), (height - 14.0).max(0.0)));
                    let headers = ["col.busid", "col.vidpid", "col.device", "col.state", "col.action"];
                    let metrics = super::window_layout::table_metrics(ui.available_width());
                    let clip_text = |ui: &mut egui::Ui, text: &str, color: Option<egui::Color32>| {
                        super::window_layout::centered_cell_label(ui, text, color, false);
                    };
                    super::window_layout::apply_table_column_spacing(ui);
                    super::window_layout::with_optional_hscroll(
                        ui,
                        metrics,
                        "devices_hscroll",
                        (height - 14.0).max(0.0),
                        |ui| {
                            super::window_layout::apply_table_column_spacing(ui);
                            let mut table = TableBuilder::new(ui)
                                .id_salt("devices")
                                .striped(true)
                                .resizable(false)
                                .auto_shrink([false, false])
                                .cell_layout(super::window_layout::cell_center_layout())
                                .min_scrolled_height(0.0)
                                .max_scroll_height((height - 46.0).max(0.0));
                            for width in metrics.widths {
                                table = table.column(Column::exact(width).clip(true));
                            }
                            table
                                .header(super::window_layout::TABLE_HEADER_HEIGHT, |mut h| {
                                    for k in headers {
                                        h.col(|ui| {
                                            super::window_layout::centered_cell_label(ui, &tr.tr(k), None, true);
                                        });
                                    }
                                })
                                .body(|mut body| {
                                    let visible = device_filter::visible_devices(
                                        &self.devices,
                                        &self.filter_rules,
                                        self.filter_show_all,
                                    );
                                    if visible.is_empty() && !self.devices.is_empty() {
                                        body.row(super::window_layout::TABLE_ROW_HEIGHT, |mut row| {
                                            row.col(|_| {});
                                            row.col(|_| {});
                                            row.col(|ui| {
                                                super::window_layout::centered_cell_label(
                                                    ui,
                                                    &tr.tr("filter.all_hidden"),
                                                    None,
                                                    false,
                                                );
                                            });
                                            row.col(|_| {});
                                            row.col(|_| {});
                                        });
                                    }
                                    for dev in visible {
                                        body.row(super::window_layout::TABLE_ROW_HEIGHT, |mut row| {
                                            row.col(|ui| clip_text(ui, &dev.busid, None));
                                            row.col(|ui| {
                                                let vp = dev.vidpid();
                                                clip_text(ui, if vp.is_empty() { "-" } else { vp.as_str() }, None);
                                            });
                                            row.col(|ui| {
                                                let desc = if !dev.description.is_empty() {
                                                    dev.description.clone()
                                                } else {
                                                    dev.instance_id.clone().unwrap_or_else(|| "-".into())
                                                };
                                                clip_text(ui, &desc, None);
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
                                                clip_text(ui, &s, Some(theme::state_color(dev.state, dark)));
                                            });
                                            row.col(|ui| {
                                                super::window_layout::centered_cell_group(ui, |ui| {
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
                                        });
                                    }
                                });
                        },
                    );
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
        let size = ui.available_size();
        super::window_layout::fixed_region(ui, size, |ui| {
            egui::Frame::new()
                .fill(theme::log_bg(dark))
                .stroke(egui::Stroke::new(1.0, theme::border(dark)))
                .corner_radius(4.0)
                .inner_margin(4.0)
                .show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    egui::ScrollArea::vertical()
                        .id_salt("log_scroll")
                        .stick_to_bottom(true)
                        .min_scrolled_height(0.0)
                        .max_height(ui.available_height().max(0.0))
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
        });
    }


    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(front) = self.dialogs.front_mut() else { return };
        let tr = &self.tr;
        // 返回值：Some(true)=确认/接受，Some(false)=取消
        let result: Option<bool> = egui::Modal::new(egui::Id::unique("usbipd_message_modal"))
            .show(ctx, |ui| match front {
                Dialog::Fatal(t) | Dialog::Warn(t) => {
                    super::dialog_layout::message(ui, t);
                    let mut r = None;
                    super::dialog_layout::buttons(ui, |ui| {
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
                    super::dialog_layout::message(ui, t);
                    let mut r = None;
                    super::dialog_layout::buttons(ui, |ui| {
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
        }
    }

    fn show_settings_window(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.settings.take() else {
            self.settings_ready = false;
            return;
        };
        let title = self.tr.tr("settings.title");
        let first = !self.settings_ready;
        let pos = self.settings_pos;
        let dark = self.dark;
        let distros_loading = self.distros_loading;
        let distros_error = self.distros_error.clone();
        let tr = &self.tr;
        let distros = &self.distros;
        let outcome = show_tool_window(
            ctx,
            settings_viewport_id(),
            tool_window(&title, super::dialog_layout::SETTINGS_WINDOW_INNER, super::dialog_layout::SETTINGS_WINDOW_MIN, pos, first),
            |ui| {
                let mut outcome = dlg.show(ui, tr, distros, distros_loading, distros_error.as_deref(), dark);
                hug_content_height(ui, dlg.content_bottom, super::dialog_layout::SETTINGS_WINDOW_MIN[1]);
                if outcome.is_none() && ui.input(|i| i.viewport().close_requested()) {
                    outcome = Some(SettingsOutcome::Cancel);
                }
                outcome
            },
        );
        self.settings_ready = true;
        match outcome {
            Some(SettingsOutcome::Confirm) => {
                dlg.prefs.apply_to_config(&mut self.cfg);
                self.cfg.save();
                self.close_settings();
            }
            Some(SettingsOutcome::Cancel) => self.close_settings(),
            None => self.settings = Some(dlg),
        }
    }

    fn close_settings(&mut self) {
        self.settings = None;
        self.settings_ready = false;
        self.settings_pos = None;
    }

    fn show_filters_window(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.filters.take() else {
            self.filters_ready = false;
            return;
        };
        dlg.show_all = self.filter_show_all;
        let title = self.tr.tr("filter.title");
        let first = !self.filters_ready;
        let pos = self.filters_pos;
        let hidden = device_filter::hidden_count(&self.devices, &self.filter_rules);
        let total = self.devices.len();
        let tr = &self.tr;
        let outcome = show_tool_window(
            ctx,
            filters_viewport_id(),
            tool_window(&title, super::dialog_layout::FILTERS_WINDOW_INNER, super::dialog_layout::FILTERS_WINDOW_MIN, pos, first),
            |ui| {
                let mut outcome = dlg.show(ui, tr, hidden, total);
                hug_content_height(ui, dlg.content_bottom, super::dialog_layout::FILTERS_WINDOW_MIN[1]);
                if outcome.is_none() && ui.input(|i| i.viewport().close_requested()) {
                    outcome = Some(FilterOutcome::Cancel);
                }
                outcome
            },
        );
        self.filter_show_all = dlg.show_all;
        self.filters_ready = true;
        match outcome {
            Some(FilterOutcome::Save) => {
                // Save already validated in the window. Display-only: no USB ops.
                let rules = dlg.take_rules();
                device_filter::store_rules(&mut self.cfg, &rules);
                self.cfg.save();
                self.filter_rules = rules;
                self.close_filters();
            }
            Some(FilterOutcome::Cancel) => self.close_filters(),
            None => self.filters = Some(dlg),
        }
    }

    fn close_filters(&mut self) {
        self.filters = None;
        self.filters_ready = false;
        self.filters_pos = None;
    }
    fn save_intents(&mut self) {
        device_intent::store_intents(&mut self.cfg, &self.intents);
        self.cfg.save();
    }

    fn refresh_intent_identities(&mut self) {
        let before = self.intents.clone();
        device_intent::refresh_identity(&mut self.intents, &self.devices);
        if self.intents != before {
            self.save_intents();
        }
    }

    fn commit_user_intent(&mut self, busid: &str) {
        let Some((_, desired)) = self.pending_user_intent.take() else {
            return;
        };
        let Some(dev) = self.devices.iter().find(|d| d.busid == busid).cloned() else {
            return;
        };
        let prefs = AttachPrefs::from_config(&self.cfg);
        let incoming = DeviceIntent::from_device(&dev, desired, &prefs);
        if let Err(reason) = device_intent::record_success(&mut self.intents, incoming.clone()) {
            self.log(&reason.log_message(&incoming));
            return;
        }
        self.save_intents();
    }

    fn restore_backoff_success(&mut self, busid: &str) {
        if let Some(dev) = self.devices.iter().find(|d| d.busid == busid) {
            let key = device_intent::backoff_key(&DeviceIntent::from_device(dev, DesiredState::Attached, &AttachPrefs::from_config(&self.cfg)));
            self.restore_backoff.entry(key).or_default().on_success();
        }
    }

    fn restore_backoff_failure(&mut self, busid: &str) {
        let now = std::time::Instant::now();
        if let Some(dev) = self.devices.iter().find(|d| d.busid == busid) {
            let key = device_intent::backoff_key(&DeviceIntent::from_device(dev, DesiredState::Attached, &AttachPrefs::from_config(&self.cfg)));
            self.restore_backoff.entry(key).or_default().on_failure(now);
        }
        self.log(&format!("[restore] failed for {busid}; will retry with backoff, intent unchanged"));
    }

    fn maybe_restore_intents(&mut self) {
        if self.busy {
            return;
        }
        let running = self.auto.running_busids();
        let now = std::time::Instant::now();
        let plan = device_intent::reconcile(&self.intents, &self.devices, &running);
        let mut skip_logs = Vec::new();
        let mut next: Option<(RestoreAction, DeviceIntent)> = None;
        for item in &plan {
            match &item.decision {
                ReconcileDecision::Skip(reason) => {
                    if matches!(
                        reason,
                        device_intent::SkipReason::Ambiguous | device_intent::SkipReason::MissingIdentity
                    ) {
                        skip_logs.push(reason.log_message(item.intent));
                    }
                }
                ReconcileDecision::Act(action) => {
                    let key = device_intent::backoff_key(item.intent);
                    if !self.restore_backoff.entry(key).or_default().allows(now) {
                        continue;
                    }
                    next = Some((action.clone(), item.intent.clone()));
                    break;
                }
            }
        }
        for line in skip_logs {
            self.log(&line);
        }
        let Some((action, intent)) = next else {
            return;
        };
        self.restore_in_flight = Some(action.clone());
        match action {
            RestoreAction::Bind { busid } => {
                self.log(&format!(
                    "[restore] non-force bind {busid} to recover remembered {}",
                    intent.desired.as_str()
                ));
                self.do_bind(&busid, false);
            }
            RestoreAction::Attach { busid } => {
                let prefs = intent.attach_prefs();
                match prefs.attach_args(&busid) {
                    Ok(args) => {
                        self.log(&format!("[restore] attach {busid} with --auto-attach (remembered attached)"));
                        self.start_attach(&busid, args, true);
                    }
                    Err(e) => {
                        self.restore_in_flight = None;
                        let msg = self.tr.tr(e.i18n_key());
                        self.warn(msg);
                    }
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

        egui::Panel::top("top_bar").show_separator_line(false).show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(egui::Image::from_texture((self.logo.id(), egui::vec2(40.0, 40.0))));
                ui.vertical(|ui| {
                    ui.heading("UsbipdTool");
                    ui.add(egui::Label::new(RichText::new(self.info_text()).small()).wrap());
                });
                // Busy / scan / no-device text lives in the existing 40 px header row so it
                // cannot steal height from the table when Bind or other actions start.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.set_min_height(40.0);
                    ui.set_max_height(40.0);
                    if self.busy {
                        ui.spinner();
                    }
                    if let Some(text) = self.status_text() {
                        ui.label(RichText::new(format!("● {text}"))
                            .color(theme::state_color(if self.busy { usbipd_tool::core::models::DeviceState::NotShared } else { usbipd_tool::core::models::DeviceState::Shared }, self.dark)));
                    }
                    // Hidden-count / show-all live in this reserved 40 px row (no extra footer).
                    let hidden = device_filter::hidden_count(&self.devices, &self.filter_rules);
                    if hidden > 0 {
                        if self.filter_show_all {
                            if ui.button(self.tr.tr("filter.resume")).clicked() {
                                self.filter_show_all = false;
                            }
                            ui.label(RichText::new(self.tr.tr("filter.showing_all")).small());
                        } else {
                            if ui.button(self.tr.tr("filter.show_all")).clicked() {
                                self.filter_show_all = true;
                            }
                            ui.label(RichText::new(self.tr.trf("filter.hidden_count", &[("n", &hidden.to_string())])).small());
                        }
                    }
                });
            });
            ui.add_space(6.0);
            self.toolbar(ui);
            ui.add_space(2.0);
        });

        let mut acts = Vec::new();
        egui::CentralPanel::default()
            .frame(super::window_layout::central_content_frame(ui.style()))
            .show(ui, |ui| {
            let avail = ui.available_height();
            let table_h = super::window_layout::table_height(avail);
            self.device_table(ui, table_h, &mut acts);
            ui.add_space(4.0);
            self.log_panel(ui);
        });
        for a in acts {
            self.handle_ui_action(a);
        }

        self.show_settings_window(&ctx);
        self.show_filters_window(&ctx);
        self.dialogs(&ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Stop --auto-attach children so they do not linger; persisted intent is kept
        // so the next startup can restore remembered devices.
        self.auto.stop_all();
        // 关闭隐藏的 USB 监听窗口
        self._hotplug = None;
    }
}
