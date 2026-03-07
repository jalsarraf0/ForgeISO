use std::path::PathBuf;
use std::sync::{mpsc, Arc};

use egui::{Color32, Frame, RichText, Stroke, Ui, Vec2};
use forgeiso_engine::{
    BuildConfig, ContainerConfig, Distro, FirewallConfig, ForgeIsoEngine, GrubConfig, InjectConfig,
    IsoSource, NetworkConfig, ProfileKind, ProxyConfig, SshConfig, SwapConfig, UserConfig,
};
use serde::{Deserialize, Serialize};

use crate::state::{
    lines, opt, BuildResult, BuildState, DiffFilter, DiffState, DoctorReport, InjectState,
    Iso9660Compliance, IsoDiff, IsoMetadata, LogEntry, LogLevel, PickTarget, Stage, StatusMsg,
    VerifyResult, VerifyState,
};
use crate::worker::{self, WorkerMsg};

// ── Persisted form state (saved across sessions via eframe storage) ──────────

const STORAGE_KEY: &str = "forgeiso_v1";

#[derive(Default, Serialize, Deserialize)]
struct PersistedState {
    inject: InjectState,
    verify: VerifyState,
    diff: DiffState,
    build: BuildState,
}

// ── Palette ───────────────────────────────────────────────────────────────────

const BG: Color32 = Color32::from_rgb(13, 17, 23);
const CARD: Color32 = Color32::from_rgb(22, 27, 42);
const CARD_BORDER: Color32 = Color32::from_rgb(40, 50, 75);
const ACCENT: Color32 = Color32::from_rgb(59, 130, 246);
const GREEN: Color32 = Color32::from_rgb(34, 197, 94);
const RED: Color32 = Color32::from_rgb(239, 68, 68);
const AMBER: Color32 = Color32::from_rgb(245, 158, 11);
const TEXT: Color32 = Color32::from_rgb(248, 250, 252);
const MUTED: Color32 = Color32::from_rgb(100, 116, 139);
const SIDEBAR: Color32 = Color32::from_rgb(17, 22, 35);

// ── Free-function UI helpers (avoid self borrow conflicts) ────────────────────

fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, CARD_BORDER))
        .inner_margin(16.0f32)
        .corner_radius(8.0f32)
        .show(ui, add);
    ui.add_space(12.0);
}

fn card_green(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(Color32::from_rgb(14, 36, 24))
        .stroke(Stroke::new(1.0, Color32::from_rgb(34, 100, 60)))
        .inner_margin(16.0f32)
        .corner_radius(8.0f32)
        .show(ui, add);
    ui.add_space(12.0);
}

fn stage_header(ui: &mut Ui, step: usize, title: &str, desc: &str) {
    Frame::new()
        .fill(Color32::from_rgb(15, 20, 36))
        .stroke(Stroke::new(1.0, Color32::from_rgb(35, 45, 75)))
        .inner_margin(12.0f32)
        .corner_radius(8.0f32)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Step {step}"))
                        .size(11.0)
                        .color(ACCENT)
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label(RichText::new(title).size(18.0).strong().color(TEXT));
            });
            ui.add_space(4.0);
            ui.label(RichText::new(desc).size(13.0).color(MUTED));
        });
    ui.add_space(12.0);
}

fn primary_btn(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    let btn = egui::Button::new(RichText::new(label).color(Color32::WHITE).strong())
        .fill(if enabled {
            ACCENT
        } else {
            Color32::from_rgb(40, 50, 80)
        })
        .min_size(Vec2::new(140.0, 36.0));
    ui.add_enabled(enabled, btn).clicked()
}

fn ghost_btn(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    let btn = egui::Button::new(label)
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0, CARD_BORDER))
        .min_size(Vec2::new(80.0, 28.0));
    ui.add_enabled(enabled, btn).clicked()
}

fn continue_btn(ui: &mut Ui, label: &str) -> bool {
    let btn = egui::Button::new(RichText::new(label).color(Color32::WHITE).strong())
        .fill(GREEN)
        .min_size(Vec2::new(180.0, 36.0));
    ui.add(btn).clicked()
}

fn badge(ui: &mut Ui, label: &str, fill: Color32, text_col: Color32) {
    Frame::new()
        .fill(fill)
        .inner_margin(Vec2::new(8.0, 4.0))
        .corner_radius(4.0f32)
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(12.0).color(text_col).strong());
        });
}

fn now_ts() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn distro_label(d: &forgeiso_engine::Distro) -> String {
    use forgeiso_engine::Distro;
    match d {
        Distro::Ubuntu => "Ubuntu".into(),
        Distro::Fedora => "Fedora".into(),
        Distro::Arch => "Arch Linux".into(),
        Distro::Mint => "Linux Mint".into(),
    }
}

fn fmt_bytes(n: u64) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else if n < 1024 * 1024 * 1024 {
        format!("{:.1} MB", n as f64 / 1_048_576.0)
    } else {
        format!("{:.2} GB", n as f64 / 1_073_741_824.0)
    }
}

// ── ForgeApp ──────────────────────────────────────────────────────────────────

pub struct ForgeApp {
    rt: tokio::runtime::Runtime,
    engine: Arc<ForgeIsoEngine>,
    tx: mpsc::Sender<WorkerMsg>,
    rx: mpsc::Receiver<WorkerMsg>,
    // Navigation
    active_stage: Stage,
    // Job
    job_running: bool,
    job_phase: String,
    job_pct: Option<f32>,
    current_task: Option<tokio::task::JoinHandle<()>>,
    // Forms
    inject: InjectState,
    verify: VerifyState,
    diff: DiffState,
    build: BuildState,
    // Results
    inject_result: Option<BuildResult>,
    verify_result: Option<VerifyResult>,
    iso9660_result: Option<Iso9660Compliance>,
    diff_result: Option<IsoDiff>,
    build_result: Option<BuildResult>,
    inspect_result: Option<IsoMetadata>,
    // Stage done flags
    inject_done: bool,
    verify_done: bool,
    diff_done: bool,
    build_done: bool,
    // Log
    log_entries: Vec<LogEntry>,
    log_open: bool,
    log_errors_only: bool,
    // Status
    status: Option<StatusMsg>,
    // Diff UI
    diff_filter: DiffFilter,
    diff_search: String,
    // Doctor
    doctor_result: Option<DoctorReport>,
    doctor_open: bool,
    // Auto-dismiss status
    status_since: Option<std::time::Instant>,
}

impl ForgeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: tokio::runtime::Runtime) -> Self {
        // Dark theme with custom palette
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = BG;
        visuals.panel_fill = BG;
        visuals.widgets.noninteractive.bg_fill = CARD;
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 38, 60);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 50, 80);
        visuals.widgets.active.bg_fill = ACCENT;
        visuals.selection.bg_fill = Color32::from_rgba_premultiplied(59, 130, 246, 80);
        visuals.override_text_color = Some(TEXT);
        cc.egui_ctx.set_visuals(visuals);

        // Increase font sizes
        let mut style = (*cc.egui_ctx.style()).clone();
        use egui::{FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Heading, FontId::proportional(20.0)),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        cc.egui_ctx.set_style(style);

        let (tx, rx) = mpsc::channel();
        let engine = Arc::new(ForgeIsoEngine::new());

        // Load persisted form state if available
        let persisted: PersistedState = cc
            .storage
            .and_then(|s| eframe::get_value(s, STORAGE_KEY))
            .unwrap_or_default();

        // Pipe engine broadcast events → mpsc channel
        {
            let mut ev_rx = engine.subscribe();
            let tx2 = tx.clone();
            rt.spawn(async move {
                while let Ok(ev) = ev_rx.recv().await {
                    use forgeiso_engine::EventLevel;
                    let is_error = matches!(ev.level, EventLevel::Error);
                    let is_warn = matches!(ev.level, EventLevel::Warn);
                    let _ = tx2.send(WorkerMsg::EngineEvent {
                        phase: format!("{:?}", ev.phase),
                        message: ev.message.clone(),
                        percent: ev.percent.map(|p| p / 100.0),
                        is_error,
                        is_warn,
                    });
                }
            });
        }

        Self {
            rt,
            engine,
            tx,
            rx,
            active_stage: Stage::Inject,
            job_running: false,
            job_phase: String::new(),
            job_pct: None,
            current_task: None,
            inject: persisted.inject,
            verify: persisted.verify,
            diff: persisted.diff,
            build: persisted.build,
            inject_result: None,
            verify_result: None,
            iso9660_result: None,
            diff_result: None,
            build_result: None,
            inspect_result: None,
            inject_done: false,
            verify_done: false,
            diff_done: false,
            build_done: false,
            log_entries: Vec::new(),
            log_open: false,
            log_errors_only: false,
            status: None,
            diff_filter: DiffFilter::All,
            diff_search: String::new(),
            doctor_result: None,
            doctor_open: false,
            status_since: None,
        }
    }

    // ── Message draining ───────────────────────────────────────────────────────

    fn drain_messages(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            self.handle_msg(msg);
            ctx.request_repaint();
        }
        // Auto-dismiss non-error status after 8s
        if let Some(t) = self.status_since {
            if t.elapsed().as_secs() >= 8
                && self.status.as_ref().map(|s| !s.is_error).unwrap_or(false)
            {
                self.status = None;
                self.status_since = None;
            }
        }
    }

    fn handle_msg(&mut self, msg: WorkerMsg) {
        match msg {
            WorkerMsg::EngineEvent {
                phase,
                message,
                percent,
                is_error,
                is_warn,
            } => {
                self.job_phase = phase.clone();
                self.job_pct = percent;
                self.log_entries.push(LogEntry {
                    phase,
                    message,
                    level: if is_error {
                        LogLevel::Error
                    } else if is_warn {
                        LogLevel::Warn
                    } else {
                        LogLevel::Info
                    },
                    timestamp: now_ts(),
                });
            }
            WorkerMsg::InjectOk(r) => {
                self.inject_done = true;
                if let Some(path) = r.artifacts.first() {
                    let s = path.to_string_lossy().into_owned();
                    if self.verify.source.is_empty() {
                        self.verify.source = s.clone();
                    }
                    if self.diff.base.is_empty() {
                        self.diff.base = self.inject.source.clone();
                    }
                    if self.diff.target.is_empty() {
                        self.diff.target = s.clone();
                    }
                    if self.build.source.is_empty() {
                        self.build.source = s;
                    }
                }
                self.inject_result = Some(*r);
                self.job_running = false;
                self.set_status(StatusMsg::ok("Inject complete"));
            }
            WorkerMsg::VerifyOk(r) => {
                let matched = r.matched;
                self.verify_result = Some(*r);
                self.verify_done = true;
                self.job_running = false;
                self.set_status(if matched {
                    StatusMsg::ok("Checksum matched")
                } else {
                    StatusMsg::err("Checksum MISMATCH")
                });
            }
            WorkerMsg::Iso9660Ok(r) => {
                let ok = r.compliant;
                self.iso9660_result = Some(*r);
                self.job_running = false;
                self.set_status(if ok {
                    StatusMsg::ok("ISO-9660 compliant")
                } else {
                    StatusMsg::err("ISO-9660 non-compliant")
                });
            }
            WorkerMsg::DiffOk(r) => {
                let total = r.added.len() + r.removed.len() + r.modified.len();
                self.diff_result = Some(*r);
                self.diff_done = true;
                self.job_running = false;
                self.set_status(StatusMsg::ok(format!(
                    "Diff complete — {total} changed files"
                )));
            }
            WorkerMsg::BuildOk(r) => {
                let path = r
                    .artifacts
                    .first()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.build_result = Some(*r);
                self.build_done = true;
                self.job_running = false;
                self.set_status(StatusMsg::ok(format!("Build complete: {path}")));
            }
            WorkerMsg::InspectOk(m) => {
                self.inspect_result = Some(*m);
                self.job_running = false;
                self.set_status(StatusMsg::ok("Inspection complete"));
            }
            WorkerMsg::DoctorOk(r) => {
                self.doctor_result = Some(*r);
                self.job_running = false;
                self.set_status(StatusMsg::ok("Doctor check complete"));
            }
            WorkerMsg::ScanOk => {
                self.job_running = false;
                self.set_status(StatusMsg::ok("Scan complete"));
            }
            WorkerMsg::TestOk => {
                self.job_running = false;
                self.set_status(StatusMsg::ok("Boot test complete"));
            }
            WorkerMsg::ReportOk(path) => {
                self.job_running = false;
                self.set_status(StatusMsg::ok(format!("Report: {path}")));
            }
            WorkerMsg::FilePicked { target, path } => match target {
                PickTarget::InjectSource => self.inject.source = path,
                PickTarget::InjectOutputDir => self.inject.output_dir = path,
                PickTarget::InjectWallpaper => self.inject.wallpaper_path = path,
                PickTarget::VerifySource => self.verify.source = path,
                PickTarget::DiffBase => self.diff.base = path,
                PickTarget::DiffTarget => self.diff.target = path,
                PickTarget::BuildSource => self.build.source = path,
                PickTarget::BuildOutputDir => self.build.output_dir = path,
                PickTarget::BuildOverlay => self.build.overlay_dir = path,
            },
            WorkerMsg::OpError(e) => {
                self.job_running = false;
                self.log_entries.push(LogEntry {
                    phase: "Error".into(),
                    message: e.clone(),
                    level: LogLevel::Error,
                    timestamp: now_ts(),
                });
                self.set_status(StatusMsg::err(e));
            }
            WorkerMsg::Done => {
                self.job_running = false;
            }
        }
    }

    // ── Engine spawn helpers ───────────────────────────────────────────────────

    fn start_job(&mut self, phase: &str) {
        self.job_running = true;
        self.job_phase = phase.into();
        self.job_pct = None;
        self.status = None;
        self.status_since = None;
    }

    fn set_status(&mut self, msg: StatusMsg) {
        self.status_since = Some(std::time::Instant::now());
        self.status = Some(msg);
    }

    fn cancel_job(&mut self) {
        if let Some(handle) = self.current_task.take() {
            handle.abort();
        }
        self.job_running = false;
        self.job_pct = None;
        self.set_status(StatusMsg::ok("Cancelled"));
    }

    fn spawn_inject(&mut self) {
        self.start_job("Injecting autoinstall…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let inject = self.inject.clone();
        let out = PathBuf::from(&inject.output_dir);
        self.current_task = Some(self.rt.spawn(async move {
            let cfg = build_inject_config(&inject);
            match engine.inject_autoinstall(&cfg, &out).await {
                Ok(r) => {
                    let _ = tx.send(WorkerMsg::InjectOk(Box::new(r)));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_verify(&mut self) {
        self.start_job("Verifying checksum…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let source = self.verify.source.clone();
        let sums = opt(&self.verify.sums_url);
        self.current_task = Some(self.rt.spawn(async move {
            match engine.verify(&source, sums.as_deref()).await {
                Ok(r) => {
                    let _ = tx.send(WorkerMsg::VerifyOk(Box::new(r)));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_iso9660(&mut self) {
        self.start_job("Validating ISO-9660…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let source = self.verify.source.clone();
        self.current_task = Some(self.rt.spawn(async move {
            match engine.validate_iso9660(&source).await {
                Ok(r) => {
                    let _ = tx.send(WorkerMsg::Iso9660Ok(Box::new(r)));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_diff(&mut self) {
        self.start_job("Comparing ISOs…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let base = PathBuf::from(&self.diff.base);
        let target = PathBuf::from(&self.diff.target);
        self.current_task = Some(self.rt.spawn(async move {
            match engine.diff_isos(&base, &target).await {
                Ok(r) => {
                    let _ = tx.send(WorkerMsg::DiffOk(Box::new(r)));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_build(&mut self) {
        self.start_job("Building ISO…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let b = self.build.clone();
        let out = PathBuf::from(&b.output_dir);
        self.current_task = Some(self.rt.spawn(async move {
            let cfg = BuildConfig {
                name: b.build_name.clone(),
                source: IsoSource::from_raw(&b.source),
                overlay_dir: opt(&b.overlay_dir).map(PathBuf::from),
                output_label: opt(&b.output_label),
                profile: if b.profile == "desktop" {
                    ProfileKind::Desktop
                } else {
                    ProfileKind::Minimal
                },
                auto_scan: false,
                auto_test: false,
                scanning: Default::default(),
                testing: Default::default(),
                keep_workdir: false,
            };
            match engine.build(&cfg, &out).await {
                Ok(r) => {
                    let _ = tx.send(WorkerMsg::BuildOk(Box::new(r)));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_inspect(&mut self) {
        self.start_job("Inspecting ISO…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let source = self.build.source.clone();
        self.current_task = Some(self.rt.spawn(async move {
            match engine.inspect_source(&source, None).await {
                Ok(m) => {
                    let _ = tx.send(WorkerMsg::InspectOk(Box::new(m)));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_doctor(&mut self) {
        self.start_job("Running dependency check…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        self.current_task = Some(self.rt.spawn(async move {
            let r = engine.doctor().await;
            let _ = tx.send(WorkerMsg::DoctorOk(Box::new(r)));
        }));
    }

    fn spawn_scan(&mut self) {
        self.start_job("Scanning artifact…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let iso = self
            .build_result
            .as_ref()
            .and_then(|r| r.artifacts.first().cloned())
            .unwrap_or_default();
        let out = iso
            .parent()
            .map(|p| p.join("scan"))
            .unwrap_or_else(|| PathBuf::from("scan"));
        self.current_task = Some(self.rt.spawn(async move {
            match engine.scan(&iso, None, &out).await {
                Ok(_) => {
                    let _ = tx.send(WorkerMsg::ScanOk);
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_test_iso(&mut self) {
        self.start_job("Running boot test…");
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let iso = self
            .build_result
            .as_ref()
            .and_then(|r| r.artifacts.first().cloned())
            .unwrap_or_default();
        let out = iso
            .parent()
            .map(|p| p.join("test"))
            .unwrap_or_else(|| PathBuf::from("test"));
        self.current_task = Some(self.rt.spawn(async move {
            match engine.test_iso(&iso, true, true, &out).await {
                Ok(_) => {
                    let _ = tx.send(WorkerMsg::TestOk);
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    fn spawn_report(&mut self, format: &str) {
        self.start_job(&format!("Rendering {format} report…"));
        let engine = Arc::clone(&self.engine);
        let tx = self.tx.clone();
        let build_dir = self
            .build_result
            .as_ref()
            .map(|r| r.output_dir.clone())
            .unwrap_or_default();
        let fmt = format.to_string();
        self.current_task = Some(self.rt.spawn(async move {
            match engine.report(&build_dir, &fmt).await {
                Ok(p) => {
                    let _ = tx.send(WorkerMsg::ReportOk(p.to_string_lossy().into_owned()));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::OpError(e.to_string()));
                }
            }
        }));
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    fn render_doctor_panel(&mut self, ctx: &egui::Context) {
        egui::Window::new("🩺 System Dependencies")
            .collapsible(false)
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| {
                if self.job_running && self.doctor_result.is_none() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            RichText::new("Checking dependencies…")
                                .size(13.0)
                                .color(MUTED),
                        );
                    });
                    return;
                }
                if let Some(ref r) = self.doctor_result.clone() {
                    // OS row
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Host").size(12.0).color(MUTED));
                        ui.label(
                            RichText::new(format!("{} / {}", r.host_os, r.host_arch))
                                .size(12.0)
                                .color(TEXT)
                                .monospace(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (col, lbl) = if r.linux_supported {
                                (GREEN, "Supported")
                            } else {
                                (AMBER, "Limited")
                            };
                            badge(
                                ui,
                                lbl,
                                if r.linux_supported {
                                    Color32::from_rgb(14, 36, 24)
                                } else {
                                    Color32::from_rgb(36, 28, 10)
                                },
                                col,
                            );
                        });
                    });
                    ui.separator();
                    ui.add_space(6.0);

                    // Tools grid
                    let mut tools: Vec<(&str, bool)> =
                        r.tooling.iter().map(|(k, v)| (k.as_str(), *v)).collect();
                    tools.sort_by_key(|(k, _)| *k);

                    let descriptions: std::collections::HashMap<&str, &str> = [
                        ("xorriso", "ISO read/write (required)"),
                        ("unsquashfs", "SquashFS extraction"),
                        ("mksquashfs", "SquashFS packaging"),
                        ("qemu-system-x86_64", "VM boot testing"),
                        ("trivy", "CVE scanner"),
                        ("syft", "SBOM generator"),
                        ("grype", "Vulnerability scanner"),
                        ("oscap", "OpenSCAP compliance"),
                    ]
                    .into_iter()
                    .collect();

                    egui::Grid::new("doctor_grid")
                        .num_columns(3)
                        .spacing([8.0, 6.0])
                        .show(ui, |ui| {
                            for (tool, ok) in &tools {
                                ui.label(if *ok {
                                    RichText::new("✓").color(GREEN)
                                } else {
                                    RichText::new("✗").color(RED)
                                });
                                ui.label(RichText::new(*tool).size(13.0).color(TEXT).monospace());
                                let desc = descriptions.get(tool).copied().unwrap_or("");
                                ui.label(RichText::new(desc).size(11.0).color(MUTED));
                                ui.end_row();
                            }
                        });

                    if !r.warnings.is_empty() {
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);
                        for w in &r.warnings {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("⚠").color(AMBER));
                                ui.label(RichText::new(w).size(12.0).color(AMBER));
                            });
                        }
                    }

                    ui.add_space(8.0);
                    if !self.job_running && ui.button("Re-run check").clicked() {
                        self.spawn_doctor();
                    }
                    let ts_display = chrono::DateTime::parse_from_rfc3339(&r.timestamp)
                        .map(|t| {
                            t.with_timezone(&chrono::Local)
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string()
                        })
                        .unwrap_or_else(|_| r.timestamp.clone());
                    ui.label(
                        RichText::new(format!("Checked at {ts_display}"))
                            .size(10.0)
                            .color(MUTED),
                    );
                } else {
                    ui.label(RichText::new("No results yet.").color(MUTED));
                    if ui.button("Run check").clicked() {
                        self.spawn_doctor();
                    }
                }
            });
    }

    fn render_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sidebar")
            .exact_width(168.0)
            .resizable(false)
            .frame(Frame::new().fill(SIDEBAR))
            .show(ctx, |ui| {
                ui.add_space(16.0);
                ui.label(RichText::new("⚙ ForgeISO").size(16.0).strong().color(TEXT));
                ui.add_space(2.0);
                ui.label(RichText::new("ISO Pipeline Wizard").size(11.0).color(MUTED));
                ui.label(
                    RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                        .size(10.0)
                        .color(Color32::from_rgb(50, 60, 80)),
                );
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(12.0);

                for stage in Stage::ALL {
                    let is_active = &self.active_stage == stage;
                    let is_done = self.stage_done(stage);
                    let dot_color = if is_done {
                        GREEN
                    } else if is_active {
                        ACCENT
                    } else {
                        MUTED
                    };
                    let bg = if is_active {
                        Color32::from_rgba_premultiplied(59, 130, 246, 25)
                    } else {
                        Color32::TRANSPARENT
                    };
                    let dot_text = if is_done {
                        "✓".to_string()
                    } else {
                        stage.step_num().to_string()
                    };

                    let resp = Frame::new()
                        .fill(bg)
                        .inner_margin(Vec2::new(8.0, 6.0))
                        .show(ui, |ui| {
                            ui.set_min_width(148.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&dot_text)
                                        .size(11.0)
                                        .color(dot_color)
                                        .strong(),
                                );
                                ui.add_space(4.0);
                                ui.vertical(|ui| {
                                    let lc = if is_active {
                                        TEXT
                                    } else {
                                        Color32::from_rgb(180, 190, 210)
                                    };
                                    ui.label(RichText::new(stage.label()).size(13.0).color(lc));
                                    ui.label(
                                        RichText::new(stage.sublabel()).size(10.0).color(MUTED),
                                    );
                                });
                            });
                        });
                    if resp.response.interact(egui::Sense::click()).clicked() {
                        self.active_stage = stage.clone();
                    }
                    ui.add_space(2.0);
                }

                // Job progress at bottom
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.add_space(8.0);
                    if self.job_running {
                        if let Some(pct) = self.job_pct {
                            ui.add(egui::ProgressBar::new(pct).desired_width(148.0));
                        }
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new(&self.job_phase).size(11.0).color(MUTED));
                        });
                    }
                    ui.separator();
                    ui.add_space(4.0);
                    // Doctor button
                    let doc_lbl = if self.doctor_open {
                        "✕ Close Doctor"
                    } else {
                        "🩺 Doctor"
                    };
                    let doc_col = if self.doctor_open { ACCENT } else { MUTED };
                    if ui
                        .add(
                            egui::Button::new(RichText::new(doc_lbl).size(12.0).color(doc_col))
                                .fill(Color32::TRANSPARENT),
                        )
                        .on_hover_text("Check system dependencies")
                        .clicked()
                    {
                        self.doctor_open = !self.doctor_open;
                        if self.doctor_open && self.doctor_result.is_none() && !self.job_running {
                            self.spawn_doctor();
                        }
                    }
                    if let Some(ref r) = self.doctor_result.clone() {
                        let ok = r.tooling.values().all(|v| *v);
                        let dot_col = if ok { GREEN } else { AMBER };
                        ui.label(RichText::new("●").size(9.0).color(dot_col));
                    }
                });
            });
    }

    fn render_log_panel(&mut self, ctx: &egui::Context) {
        let mut do_cancel = false;
        egui::TopBottomPanel::bottom("log_panel")
            .resizable(true)
            .min_height(24.0)
            .max_height(360.0)
            .default_height(if self.log_open { 120.0 } else { 24.0 })
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(10, 14, 22))
                    .stroke(Stroke::new(1.0, CARD_BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let arrow = if self.log_open { "▼" } else { "▶" };
                    let error_count = self
                        .log_entries
                        .iter()
                        .filter(|e| e.level == LogLevel::Error)
                        .count();
                    let log_lbl = if error_count > 0 {
                        format!(
                            "{arrow} Log ({} entries, {} error{})",
                            self.log_entries.len(),
                            error_count,
                            if error_count == 1 { "" } else { "s" }
                        )
                    } else {
                        format!("{arrow} Log ({} entries)", self.log_entries.len())
                    };
                    let btn_col = if error_count > 0 && !self.log_open {
                        RED
                    } else {
                        TEXT
                    };
                    if ui
                        .add(
                            egui::Button::new(RichText::new(log_lbl).size(12.0).color(btn_col))
                                .fill(Color32::TRANSPARENT),
                        )
                        .clicked()
                    {
                        self.log_open = !self.log_open;
                    }
                    if self.job_running {
                        if let Some(pct) = self.job_pct {
                            ui.add(
                                egui::ProgressBar::new(pct)
                                    .desired_width(120.0)
                                    .text(format!("{:.0}%", pct * 100.0)),
                            );
                        } else {
                            ui.spinner();
                        }
                        ui.label(RichText::new(&self.job_phase).size(11.0).color(MUTED));
                        if ghost_btn(ui, "✕ Cancel", true) {
                            do_cancel = true;
                        }
                    }
                    if self.log_open {
                        ui.add_space(8.0);
                        if ui.selectable_label(!self.log_errors_only, "All").clicked() {
                            self.log_errors_only = false;
                        }
                        if ui
                            .selectable_label(self.log_errors_only, "Errors")
                            .clicked()
                        {
                            self.log_errors_only = true;
                        }
                        if ui.small_button("Clear").clicked() {
                            self.log_entries.clear();
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(ref s) = self.status {
                            ui.label(RichText::new(&s.text).size(12.0).color(if s.is_error {
                                RED
                            } else {
                                GREEN
                            }));
                        }
                    });
                });

                if self.log_open {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for entry in &self.log_entries {
                                if self.log_errors_only && entry.level != LogLevel::Error {
                                    continue;
                                }
                                let col = match entry.level {
                                    LogLevel::Error => RED,
                                    LogLevel::Warn => AMBER,
                                    LogLevel::Info => MUTED,
                                };
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&entry.timestamp)
                                            .size(10.0)
                                            .color(Color32::from_rgb(60, 70, 90))
                                            .monospace(),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(&entry.phase)
                                            .size(11.0)
                                            .color(ACCENT)
                                            .monospace(),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(RichText::new(&entry.message).size(11.0).color(col));
                                });
                            }
                        });
                }
            });
        if do_cancel {
            self.cancel_job();
        }
    }

    fn stage_done(&self, stage: &Stage) -> bool {
        match stage {
            Stage::Inject => self.inject_done,
            Stage::Verify => self.verify_done,
            Stage::Diff => self.diff_done,
            Stage::Build => self.build_done,
            Stage::Completion => false,
        }
    }

    fn render_main(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add_space(24.0);
                        let margin = 24.0f32;
                        Frame::new()
                            .inner_margin(Vec2::new(margin, 0.0))
                            .show(ui, |ui| match self.active_stage.clone() {
                                Stage::Inject => self.show_inject(ui),
                                Stage::Verify => self.show_verify(ui),
                                Stage::Diff => self.show_diff(ui),
                                Stage::Build => self.show_build(ui),
                                Stage::Completion => self.show_completion(ui),
                            });
                        ui.add_space(32.0);
                    });
            });
    }

    // ── Stage: Inject ─────────────────────────────────────────────────────────

    fn show_inject(&mut self, ui: &mut Ui) {
        stage_header(ui, 1, "Inject", "Inject a cloud-init autoinstall configuration into an ISO image. Configure identity, SSH, network, and system settings below.");

        let running = self.job_running;
        let mut do_inject = false;

        card(ui, |ui| {
            ui.label(
                RichText::new("Source & Output")
                    .strong()
                    .size(15.0)
                    .color(TEXT),
            );
            ui.add_space(8.0);
            egui::Grid::new("inject_grid")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Source ISO *").size(12.0).color(MUTED));
                        ui.horizontal(|ui| {
                            ui.add_enabled(
                                !running,
                                egui::TextEdit::singleline(&mut self.inject.source)
                                    .hint_text("/path/to/ubuntu.iso or https://…")
                                    .desired_width(ui.available_width() - 34.0),
                            );
                            if ui
                                .add_enabled(!running, egui::Button::new("📂"))
                                .on_hover_text("Browse for ISO file")
                                .clicked()
                            {
                                worker::pick_iso(PickTarget::InjectSource, self.tx.clone());
                            }
                        });
                    });
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Output directory *").size(12.0).color(MUTED));
                        ui.horizontal(|ui| {
                            ui.add_enabled(
                                !running,
                                egui::TextEdit::singleline(&mut self.inject.output_dir)
                                    .hint_text("~/.cache/forgeiso")
                                    .desired_width(ui.available_width() - 34.0),
                            );
                            if ui.add_enabled(!running, egui::Button::new("📂")).clicked() {
                                worker::pick_folder(PickTarget::InjectOutputDir, self.tx.clone());
                            }
                        });
                    });
                    ui.end_row();
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Output filename").size(12.0).color(MUTED));
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.out_name)
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Volume label").size(12.0).color(MUTED));
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.output_label)
                                .hint_text("FORGEISO")
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.end_row();
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Distribution").size(12.0).color(MUTED));
                        egui::ComboBox::from_id_salt("inject_distro")
                            .selected_text(&self.inject.distro)
                            .show_ui(ui, |ui| {
                                for d in &["ubuntu", "fedora", "arch", "mint"] {
                                    ui.selectable_value(&mut self.inject.distro, d.to_string(), *d);
                                }
                            });
                    });
                    ui.end_row();
                });

            ui.add_space(12.0);
            ui.label(RichText::new("Identity").strong().size(15.0).color(TEXT));
            ui.add_space(6.0);
            egui::Grid::new("identity_grid")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Hostname").size(12.0).color(MUTED));
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.hostname)
                                .hint_text("my-server")
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Username").size(12.0).color(MUTED));
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.username)
                                .hint_text("admin")
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.end_row();
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Password").size(12.0).color(MUTED));
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.password)
                                .password(true)
                                .hint_text("•••••")
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Real name").size(12.0).color(MUTED));
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.realname)
                                .hint_text("John Doe")
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.end_row();
                });

            ui.add_space(12.0);
            ui.collapsing(
                RichText::new("⚙ Advanced Options").size(13.0).color(MUTED),
                |ui| {
                    // SSH
                    ui.add_space(6.0);
                    ui.label(RichText::new("SSH").strong().size(14.0).color(TEXT));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Authorized keys (one per line)")
                            .size(12.0)
                            .color(MUTED),
                    );
                    ui.add_enabled(
                        !running,
                        egui::TextEdit::multiline(&mut self.inject.ssh_keys)
                            .hint_text("ssh-rsa AAAA…")
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.inject.ssh_password_auth, "Allow password auth");
                        ui.add_space(12.0);
                        ui.checkbox(
                            &mut self.inject.ssh_install_server,
                            "Install OpenSSH server",
                        );
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Network
                    ui.label(RichText::new("Network").strong().size(14.0).color(TEXT));
                    ui.add_space(4.0);
                    egui::Grid::new("net_grid")
                        .num_columns(2)
                        .spacing([16.0, 6.0])
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("DNS servers (one/line)")
                                        .size(12.0)
                                        .color(MUTED),
                                );
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::multiline(&mut self.inject.dns_servers)
                                        .hint_text("1.1.1.1\n8.8.8.8")
                                        .desired_rows(2)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("NTP servers (one/line)")
                                        .size(12.0)
                                        .color(MUTED),
                                );
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::multiline(&mut self.inject.ntp_servers)
                                        .hint_text("pool.ntp.org")
                                        .desired_rows(2)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.end_row();
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Static IP (CIDR)").size(12.0).color(MUTED));
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::singleline(&mut self.inject.static_ip)
                                        .hint_text("192.168.1.10/24")
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Gateway").size(12.0).color(MUTED));
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::singleline(&mut self.inject.gateway)
                                        .hint_text("192.168.1.1")
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.end_row();
                        });
                    ui.end_row();
                    ui.vertical(|ui| {
                        ui.label(RichText::new("HTTP proxy").size(12.0).color(MUTED));
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.http_proxy)
                                .hint_text("http://proxy.corp.com:3128")
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.vertical(|ui| {
                        ui.label(RichText::new("HTTPS proxy").size(12.0).color(MUTED));
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.https_proxy)
                                .hint_text("https://proxy.corp.com:3128")
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // System
                    ui.label(RichText::new("System").strong().size(14.0).color(TEXT));
                    ui.add_space(4.0);
                    egui::Grid::new("sys_grid")
                        .num_columns(2)
                        .spacing([16.0, 6.0])
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Timezone").size(12.0).color(MUTED));
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::singleline(&mut self.inject.timezone)
                                        .hint_text("America/New_York")
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Locale").size(12.0).color(MUTED));
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::singleline(&mut self.inject.locale)
                                        .hint_text("en_US.UTF-8")
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.end_row();
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Keyboard layout").size(12.0).color(MUTED));
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::singleline(&mut self.inject.keyboard_layout)
                                        .hint_text("us")
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.vertical(|ui| {
                                ui.label(RichText::new("APT mirror").size(12.0).color(MUTED));
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::singleline(&mut self.inject.apt_mirror)
                                        .hint_text("http://archive.ubuntu.com/ubuntu")
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.end_row();
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Storage layout").size(12.0).color(MUTED));
                                egui::ComboBox::from_id_salt("storage_layout")
                                    .selected_text(if self.inject.storage_layout.is_empty() {
                                        "direct (default)"
                                    } else {
                                        &self.inject.storage_layout
                                    })
                                    .show_ui(ui, |ui| {
                                        for (v, l) in &[
                                            ("", "direct (default)"),
                                            ("direct", "direct"),
                                            ("lvm", "lvm"),
                                            ("zfs", "zfs"),
                                        ] {
                                            ui.selectable_value(
                                                &mut self.inject.storage_layout,
                                                v.to_string(),
                                                *l,
                                            );
                                        }
                                    });
                            });
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Wallpaper").size(12.0).color(MUTED));
                                ui.horizontal(|ui| {
                                    ui.add_enabled(
                                        !running,
                                        egui::TextEdit::singleline(&mut self.inject.wallpaper_path)
                                            .hint_text("/path/to/wallpaper.png")
                                            .desired_width(ui.available_width() - 34.0),
                                    );
                                    if ui.add_enabled(!running, egui::Button::new("📂")).clicked()
                                    {
                                        worker::pick_file(
                                            PickTarget::InjectWallpaper,
                                            self.tx.clone(),
                                        );
                                    }
                                });
                            });
                            ui.end_row();
                        });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Packages & Commands
                    ui.label(
                        RichText::new("Packages & Commands")
                            .strong()
                            .size(14.0)
                            .color(TEXT),
                    );
                    ui.add_space(4.0);
                    egui::Grid::new("pkg_grid")
                        .num_columns(2)
                        .spacing([16.0, 6.0])
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Extra packages (one/line)")
                                        .size(12.0)
                                        .color(MUTED),
                                );
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::multiline(&mut self.inject.packages)
                                        .hint_text("curl\nvim\ngit")
                                        .desired_rows(3)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("APT repos (one/line)")
                                        .size(12.0)
                                        .color(MUTED),
                                );
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::multiline(&mut self.inject.apt_repos)
                                        .hint_text("ppa:user/repo")
                                        .desired_rows(3)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.end_row();
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Run commands (one/line)")
                                        .size(12.0)
                                        .color(MUTED),
                                );
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::multiline(&mut self.inject.run_commands)
                                        .hint_text("echo hello")
                                        .desired_rows(3)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Late commands (one/line)")
                                        .size(12.0)
                                        .color(MUTED),
                                );
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::multiline(&mut self.inject.late_commands)
                                        .hint_text("curtin in-target -- apt-get upgrade -y")
                                        .desired_rows(3)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.end_row();
                        });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Firewall & Containers
                    ui.label(
                        RichText::new("Firewall & Containers")
                            .strong()
                            .size(14.0)
                            .color(TEXT),
                    );
                    ui.add_space(4.0);
                    ui.checkbox(&mut self.inject.firewall_enabled, "Enable UFW firewall");
                    if self.inject.firewall_enabled {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Default policy").size(12.0).color(MUTED));
                            ui.add_space(8.0);
                            egui::ComboBox::from_id_salt("fw_policy")
                                .selected_text(if self.inject.firewall_policy.is_empty() {
                                    "deny"
                                } else {
                                    &self.inject.firewall_policy
                                })
                                .show_ui(ui, |ui| {
                                    for p in &["deny", "allow", "reject"] {
                                        ui.selectable_value(
                                            &mut self.inject.firewall_policy,
                                            p.to_string(),
                                            *p,
                                        );
                                    }
                                });
                        });
                        ui.add_space(4.0);
                        egui::Grid::new("fw_grid")
                            .num_columns(2)
                            .spacing([16.0, 6.0])
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("Allow ports (one/line)")
                                            .size(12.0)
                                            .color(MUTED),
                                    );
                                    ui.add_enabled(
                                        !running,
                                        egui::TextEdit::multiline(&mut self.inject.allow_ports)
                                            .hint_text("22/tcp\n80/tcp")
                                            .desired_rows(2)
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("Deny ports (one/line)")
                                            .size(12.0)
                                            .color(MUTED),
                                    );
                                    ui.add_enabled(
                                        !running,
                                        egui::TextEdit::multiline(&mut self.inject.deny_ports)
                                            .hint_text("23/tcp")
                                            .desired_rows(2)
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                                ui.end_row();
                            });
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.inject.docker, "Install Docker");
                        ui.add_space(12.0);
                        ui.checkbox(&mut self.inject.podman, "Install Podman");
                        ui.add_space(12.0);
                        ui.checkbox(&mut self.inject.no_user_interaction, "No user interaction");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);
                    // Swap
                    ui.label(RichText::new("Swap").strong().size(14.0).color(TEXT));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Swap size (MiB)").size(12.0).color(MUTED));
                        ui.add_space(8.0);
                        ui.add_enabled(
                            !running,
                            egui::TextEdit::singleline(&mut self.inject.swap_size_mb)
                                .hint_text("0 = disabled, e.g. 2048")
                                .desired_width(140.0),
                        );
                        if !self.inject.swap_size_mb.trim().is_empty()
                            && self.inject.swap_size_mb.trim().parse::<u32>().is_err()
                        {
                            ui.label(RichText::new("Must be a number").size(11.0).color(RED));
                        }
                    });
                },
            );

            ui.add_space(16.0);
            let can = !self.inject.source.trim().is_empty() && !running;
            if primary_btn(
                ui,
                if running {
                    "⏳ Injecting…"
                } else {
                    "▶ Inject ISO"
                },
                can,
            ) {
                do_inject = true;
            }
        });

        if do_inject {
            self.spawn_inject();
        }

        if let Some(r) = self.inject_result.clone() {
            card_green(ui, |ui| {
                ui.label(
                    RichText::new("✓ Inject Complete")
                        .size(16.0)
                        .strong()
                        .color(GREEN),
                );
                ui.add_space(8.0);
                for a in &r.artifacts {
                    ui.horizontal(|ui| {
                        ui.label("💿");
                        ui.label(
                            RichText::new(a.to_string_lossy().as_ref())
                                .size(13.0)
                                .monospace()
                                .color(TEXT),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("📋").on_hover_text("Copy path").clicked() {
                                ui.ctx().copy_text(a.to_string_lossy().into_owned());
                            }
                            if ui.small_button("📂").on_hover_text("Open folder").clicked() {
                                if let Some(dir) = a.parent() {
                                    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
                                }
                            }
                        });
                    });
                }
                ui.add_space(10.0);
                if continue_btn(ui, "Continue to Verify →") {
                    self.active_stage = Stage::Verify;
                }
            });
        }
    }

    // ── Stage: Verify ─────────────────────────────────────────────────────────

    fn show_verify(&mut self, ui: &mut Ui) {
        stage_header(ui, 2, "Verify", "Verify the ISO checksum against official sources and confirm the image is a valid ISO-9660 filesystem. Both checks must pass before safe deployment.");

        let running = self.job_running;
        let mut do_verify = false;
        let mut do_iso9660 = false;

        card(ui, |ui| {
            ui.label(
                RichText::new("SHA-256 Checksum")
                    .strong()
                    .size(15.0)
                    .color(TEXT),
            );
            ui.add_space(4.0);
            ui.label(RichText::new("Verify an ISO against its official SHA256SUMS file. Auto-detected for Ubuntu releases.").size(12.0).color(MUTED));
            ui.add_space(10.0);

            ui.label(RichText::new("ISO path *").size(12.0).color(MUTED));
            ui.horizontal(|ui| {
                ui.add_enabled(
                    !running,
                    egui::TextEdit::singleline(&mut self.verify.source)
                        .hint_text("/path/to/ubuntu.iso or https://…")
                        .desired_width(ui.available_width() - 82.0),
                );
                if ui
                    .add_enabled(!running, egui::Button::new("📂 Browse"))
                    .clicked()
                {
                    worker::pick_iso(PickTarget::VerifySource, self.tx.clone());
                }
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new("SHA256SUMS URL (optional — auto-detected for Ubuntu)")
                    .size(12.0)
                    .color(MUTED),
            );
            ui.add_enabled(
                !running,
                egui::TextEdit::singleline(&mut self.verify.sums_url)
                    .hint_text("https://releases.ubuntu.com/24.04/SHA256SUMS")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(12.0);
            let can = !self.verify.source.trim().is_empty() && !running;
            if primary_btn(
                ui,
                if running {
                    "⏳ Verifying…"
                } else {
                    "✓ Verify Checksum"
                },
                can,
            ) {
                do_verify = true;
            }
        });

        if let Some(r) = self.verify_result.clone() {
            let (bg, border, icon) = if r.matched {
                (
                    Color32::from_rgb(14, 36, 24),
                    Color32::from_rgb(34, 100, 60),
                    "✅",
                )
            } else {
                (
                    Color32::from_rgb(36, 14, 14),
                    Color32::from_rgb(100, 34, 34),
                    "❌",
                )
            };
            Frame::new()
                .fill(bg)
                .stroke(Stroke::new(1.0, border))
                .inner_margin(16.0f32)
                .corner_radius(8.0f32)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icon).size(24.0));
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            let col = if r.matched { GREEN } else { RED };
                            ui.label(
                                RichText::new(if r.matched {
                                    "Checksum Matched"
                                } else {
                                    "Checksum MISMATCH"
                                })
                                .size(15.0)
                                .strong()
                                .color(col),
                            );
                            ui.label(RichText::new(&r.filename).size(12.0).color(MUTED));
                        });
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Expected:").size(11.0).color(MUTED));
                        ui.label(
                            RichText::new(format!("{}…", &r.expected[..32.min(r.expected.len())]))
                                .size(11.0)
                                .monospace()
                                .color(MUTED),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Actual:  ").size(11.0).color(MUTED));
                        let col = if r.matched { GREEN } else { RED };
                        ui.label(
                            RichText::new(format!("{}…", &r.actual[..32.min(r.actual.len())]))
                                .size(11.0)
                                .monospace()
                                .color(col),
                        );
                        if ui
                            .small_button("📋")
                            .on_hover_text("Copy full SHA-256")
                            .clicked()
                        {
                            ui.ctx().copy_text(r.actual.clone());
                        }
                    });
                });
            ui.add_space(12.0);
        }

        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("ISO-9660 Compliance")
                            .strong()
                            .size(15.0)
                            .color(TEXT),
                    );
                    ui.label(
                        RichText::new("Confirms valid ISO-9660 PVD and El Torito boot catalog.")
                            .size(12.0)
                            .color(MUTED),
                    );
                });
                if let Some(ref r) = self.iso9660_result {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        let (col, lbl) = if r.compliant {
                            (GREEN, "Compliant")
                        } else {
                            (RED, "Non-Compliant")
                        };
                        badge(
                            ui,
                            lbl,
                            if r.compliant {
                                Color32::from_rgb(14, 36, 24)
                            } else {
                                Color32::from_rgb(36, 14, 14)
                            },
                            col,
                        );
                    });
                }
            });
            ui.add_space(10.0);
            let can = !self.verify.source.trim().is_empty() && !running;
            if ghost_btn(
                ui,
                if running {
                    "⏳ Checking…"
                } else {
                    "Validate ISO-9660"
                },
                can,
            ) {
                do_iso9660 = true;
            }

            if let Some(ref r) = self.iso9660_result {
                ui.add_space(10.0);
                let rows: &[(bool, &str, String)] = &[
                    (
                        r.compliant,
                        "ISO-9660 PVD (CD001)",
                        if r.compliant {
                            "Confirmed at sector 16".into()
                        } else {
                            "Not found".into()
                        },
                    ),
                    (r.size_bytes > 0, "Image size", fmt_bytes(r.size_bytes)),
                    (
                        r.el_torito_present,
                        "El Torito boot catalog",
                        if r.el_torito_present {
                            "Present".into()
                        } else {
                            "Not found".into()
                        },
                    ),
                    (
                        r.boot_bios,
                        "BIOS boot entry",
                        if r.boot_bios {
                            "Detected".into()
                        } else {
                            "Not detected".into()
                        },
                    ),
                    (
                        r.boot_uefi,
                        "UEFI boot entry",
                        if r.boot_uefi {
                            "Detected".into()
                        } else {
                            "Not detected".into()
                        },
                    ),
                ];
                for (ok, label, detail) in rows {
                    ui.horizontal(|ui| {
                        ui.label(if *ok { "✅" } else { "❌" });
                        ui.label(
                            RichText::new(*label)
                                .size(13.0)
                                .color(Color32::from_rgb(180, 190, 210)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(detail).size(12.0).color(MUTED).monospace());
                        });
                    });
                    ui.separator();
                }
                ui.label(
                    RichText::new(format!("Method: {}", r.check_method))
                        .size(11.0)
                        .color(MUTED),
                );
            }
        });

        if do_verify {
            self.spawn_verify();
        }
        if do_iso9660 {
            self.spawn_iso9660();
        }

        if self.verify_result.is_some() || self.iso9660_result.is_some() {
            let can_proceed = self
                .verify_result
                .as_ref()
                .map(|r| r.matched)
                .unwrap_or(false)
                && self
                    .iso9660_result
                    .as_ref()
                    .map(|r| r.compliant)
                    .unwrap_or(false);
            card_green(ui, |ui| {
                let title = if can_proceed {
                    "✓ Verification Passed"
                } else {
                    "⚠ Verification Complete"
                };
                let col = if can_proceed { GREEN } else { AMBER };
                ui.label(RichText::new(title).size(16.0).strong().color(col));
                if !can_proceed {
                    ui.add_space(4.0);
                    ui.label(RichText::new("One or more checks did not pass. You can still continue, but deployment risk is elevated.").size(12.0).color(AMBER));
                }
                ui.add_space(10.0);
                if continue_btn(ui, "Continue to Diff →") {
                    self.verify_done = true;
                    self.active_stage = Stage::Diff;
                }
            });
        }
    }

    // ── Stage: Diff ───────────────────────────────────────────────────────────

    fn show_diff(&mut self, ui: &mut Ui) {
        stage_header(ui, 3, "Diff", "Compare the original and injected ISOs to see exactly what changed — added, removed, and modified files.");

        let running = self.job_running;
        let mut do_diff = false;

        card(ui, |ui| {
            ui.label(RichText::new("ISO Diff").strong().size(15.0).color(TEXT));
            ui.add_space(4.0);
            ui.label(
                RichText::new("Compare two ISOs to see added, removed, and modified files.")
                    .size(12.0)
                    .color(MUTED),
            );
            ui.add_space(10.0);
            egui::Grid::new("diff_grid")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Base ISO path").size(12.0).color(MUTED));
                        ui.horizontal(|ui| {
                            ui.add_enabled(
                                !running,
                                egui::TextEdit::singleline(&mut self.diff.base)
                                    .hint_text("/path/to/original.iso")
                                    .desired_width(ui.available_width() - 34.0),
                            );
                            if ui.add_enabled(!running, egui::Button::new("📂")).clicked() {
                                worker::pick_iso(PickTarget::DiffBase, self.tx.clone());
                            }
                        });
                    });
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Target ISO path").size(12.0).color(MUTED));
                        ui.horizontal(|ui| {
                            ui.add_enabled(
                                !running,
                                egui::TextEdit::singleline(&mut self.diff.target)
                                    .hint_text("/path/to/modified.iso")
                                    .desired_width(ui.available_width() - 34.0),
                            );
                            if ui.add_enabled(!running, egui::Button::new("📂")).clicked() {
                                worker::pick_iso(PickTarget::DiffTarget, self.tx.clone());
                            }
                        });
                    });
                    ui.end_row();
                });
            ui.add_space(12.0);
            let can = !self.diff.base.trim().is_empty()
                && !self.diff.target.trim().is_empty()
                && !running;
            if primary_btn(
                ui,
                if running {
                    "⏳ Comparing…"
                } else {
                    "Compare ISOs"
                },
                can,
            ) {
                do_diff = true;
            }
        });

        if do_diff {
            self.spawn_diff();
        }

        if let Some(r) = self.diff_result.clone() {
            let added = r.added.len();
            let removed = r.removed.len();
            let modified = r.modified.len();
            let unchanged = r.unchanged;
            let total = added + removed + modified;

            // Summary stats row
            egui::Grid::new("diff_stats")
                .num_columns(4)
                .spacing([8.0, 8.0])
                .show(ui, |ui| {
                    for (count, label, color) in [
                        (added, "Added", GREEN),
                        (removed, "Removed", RED),
                        (modified, "Modified", AMBER),
                        (unchanged, "Unchanged", MUTED),
                    ] {
                        Frame::new()
                            .fill(CARD)
                            .stroke(Stroke::new(1.0, CARD_BORDER))
                            .inner_margin(12.0f32)
                            .corner_radius(8.0f32)
                            .show(ui, |ui| {
                                ui.set_min_width(80.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new(count.to_string())
                                            .size(24.0)
                                            .strong()
                                            .color(color),
                                    );
                                    ui.label(RichText::new(label).size(12.0).color(MUTED));
                                });
                            });
                    }
                    ui.end_row();
                });
            ui.add_space(12.0);

            // Filter + search + list
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    for (f, lbl) in [
                        (DiffFilter::All, format!("All ({total})")),
                        (DiffFilter::Added, format!("Added ({added})")),
                        (DiffFilter::Removed, format!("Removed ({removed})")),
                        (DiffFilter::Modified, format!("Modified ({modified})")),
                    ] {
                        let active = self.diff_filter == f;
                        let btn = egui::Button::new(RichText::new(&lbl).size(12.0))
                            .fill(if active { ACCENT } else { Color32::TRANSPARENT })
                            .stroke(Stroke::new(1.0, if active { ACCENT } else { CARD_BORDER }));
                        if ui.add(btn).clicked() {
                            self.diff_filter = f;
                        }
                    }
                });
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.diff_search)
                        .hint_text("Filter by path…")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);

                let search = self.diff_search.to_lowercase();
                let filter = self.diff_filter.clone();

                let mut rows: Vec<(char, String, Option<u64>)> = Vec::new();
                if matches!(filter, DiffFilter::All | DiffFilter::Added) {
                    for p in &r.added {
                        if search.is_empty() || p.to_lowercase().contains(&search) {
                            rows.push(('A', p.clone(), None));
                        }
                    }
                }
                if matches!(filter, DiffFilter::All | DiffFilter::Removed) {
                    for p in &r.removed {
                        if search.is_empty() || p.to_lowercase().contains(&search) {
                            rows.push(('R', p.clone(), None));
                        }
                    }
                }
                if matches!(filter, DiffFilter::All | DiffFilter::Modified) {
                    for e in &r.modified {
                        if search.is_empty() || e.path.to_lowercase().contains(&search) {
                            rows.push(('M', e.path.clone(), Some(e.target_size)));
                        }
                    }
                }

                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if rows.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(12.0);
                                let msg = if self.diff_search.is_empty() {
                                    "No changes in this category"
                                } else {
                                    "No files match the search filter"
                                };
                                ui.label(RichText::new(msg).size(12.0).color(MUTED));
                            });
                        } else {
                            for (tag, path, size) in &rows {
                                let (tc, bg) = match tag {
                                    'A' => (GREEN, Color32::from_rgb(14, 36, 24)),
                                    'R' => (RED, Color32::from_rgb(36, 14, 14)),
                                    _ => (AMBER, Color32::from_rgb(36, 28, 10)),
                                };
                                ui.horizontal(|ui| {
                                    Frame::new()
                                        .fill(bg)
                                        .inner_margin(Vec2::new(6.0, 2.0))
                                        .corner_radius(3.0f32)
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(tag.to_string())
                                                    .size(11.0)
                                                    .strong()
                                                    .color(tc)
                                                    .monospace(),
                                            );
                                        });
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(path).size(12.0).monospace().color(TEXT),
                                    );
                                    if let Some(sz) = size {
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(fmt_bytes(*sz))
                                                        .size(11.0)
                                                        .color(MUTED),
                                                );
                                            },
                                        );
                                    }
                                });
                                ui.separator();
                            }
                        }
                    });
            });

            card_green(ui, |ui| {
                ui.label(
                    RichText::new("✓ Diff Complete")
                        .size(16.0)
                        .strong()
                        .color(GREEN),
                );
                ui.add_space(8.0);
                if continue_btn(ui, "Continue to Build →") {
                    self.diff_done = true;
                    self.active_stage = Stage::Build;
                }
            });
        }
    }

    // ── Stage: Build ──────────────────────────────────────────────────────────

    fn show_build(&mut self, ui: &mut Ui) {
        stage_header(
            ui,
            4,
            "Build",
            "Optionally build a custom ISO — fetch a base image, apply an overlay, and repackage.",
        );

        let running = self.job_running;
        let has_artifact = self
            .build_result
            .as_ref()
            .map(|r| !r.artifacts.is_empty())
            .unwrap_or(false);

        let mut do_build = false;
        let mut do_inspect = false;
        let mut do_scan = false;
        let mut do_test = false;
        let mut do_html = false;
        let mut do_json = false;

        // Distro selector
        card(ui, |ui| {
            ui.label(
                RichText::new("Target Distribution")
                    .strong()
                    .size(15.0)
                    .color(TEXT),
            );
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for (id, label, desc) in [
                    ("ubuntu", "Ubuntu", "cloud-init"),
                    ("fedora", "Fedora", "kickstart"),
                    ("arch", "Arch", "archinstall"),
                    ("mint", "Mint", "ubuntu-based"),
                ] {
                    let selected = self.build.distro == id;
                    let resp = Frame::new()
                        .fill(if selected {
                            Color32::from_rgba_premultiplied(59, 130, 246, 30)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .stroke(Stroke::new(
                            if selected { 2.0 } else { 1.0 },
                            if selected { ACCENT } else { CARD_BORDER },
                        ))
                        .inner_margin(12.0f32)
                        .corner_radius(8.0f32)
                        .show(ui, |ui| {
                            ui.set_min_width(100.0);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new(label).size(14.0).strong().color(TEXT));
                                ui.label(RichText::new(desc).size(11.0).color(MUTED));
                            });
                        });
                    if resp.response.interact(egui::Sense::click()).clicked() {
                        self.build.distro = id.to_string();
                    }
                    ui.add_space(4.0);
                }
            });
        });

        let avail = ui.available_width();
        let col_w = (avail - 12.0) / 2.0;

        ui.horizontal(|ui| {
            // Left: config
            ui.vertical(|ui| {
                ui.set_max_width(col_w);
                card(ui, |ui| {
                    ui.label(
                        RichText::new("Build Configuration")
                            .strong()
                            .size(15.0)
                            .color(TEXT),
                    );
                    ui.add_space(8.0);
                    egui::Grid::new("build_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Source ISO / URL *").size(12.0).color(MUTED),
                                );
                                ui.horizontal(|ui| {
                                    ui.add_enabled(
                                        !running,
                                        egui::TextEdit::singleline(&mut self.build.source)
                                            .hint_text("/path/to/ubuntu.iso or https://…")
                                            .desired_width(ui.available_width() - 34.0),
                                    );
                                    if ui.add_enabled(!running, egui::Button::new("📂")).clicked()
                                    {
                                        worker::pick_iso(PickTarget::BuildSource, self.tx.clone());
                                    }
                                });
                            });
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Output directory *").size(12.0).color(MUTED),
                                );
                                ui.horizontal(|ui| {
                                    ui.add_enabled(
                                        !running,
                                        egui::TextEdit::singleline(&mut self.build.output_dir)
                                            .hint_text("./artifacts")
                                            .desired_width(ui.available_width() - 34.0),
                                    );
                                    if ui.add_enabled(!running, egui::Button::new("📂")).clicked()
                                    {
                                        worker::pick_folder(
                                            PickTarget::BuildOutputDir,
                                            self.tx.clone(),
                                        );
                                    }
                                });
                            });
                            ui.end_row();
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Build name").size(12.0).color(MUTED));
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::singleline(&mut self.build.build_name)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Volume label").size(12.0).color(MUTED));
                                ui.add_enabled(
                                    !running,
                                    egui::TextEdit::singleline(&mut self.build.output_label)
                                        .hint_text("FORGEISO")
                                        .desired_width(f32::INFINITY),
                                );
                            });
                            ui.end_row();
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("Overlay directory").size(12.0).color(MUTED),
                                );
                                ui.horizontal(|ui| {
                                    ui.add_enabled(
                                        !running,
                                        egui::TextEdit::singleline(&mut self.build.overlay_dir)
                                            .hint_text("/path/to/overlay")
                                            .desired_width(ui.available_width() - 34.0),
                                    );
                                    if ui.add_enabled(!running, egui::Button::new("📂")).clicked()
                                    {
                                        worker::pick_folder(
                                            PickTarget::BuildOverlay,
                                            self.tx.clone(),
                                        );
                                    }
                                });
                            });
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Profile").size(12.0).color(MUTED));
                                egui::ComboBox::from_id_salt("build_profile")
                                    .selected_text(&self.build.profile)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut self.build.profile,
                                            "minimal".into(),
                                            "Minimal",
                                        );
                                        ui.selectable_value(
                                            &mut self.build.profile,
                                            "desktop".into(),
                                            "Desktop",
                                        );
                                    });
                            });
                            ui.end_row();
                        });
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let can_build = !self.build.source.trim().is_empty()
                            && !self.build.output_dir.trim().is_empty()
                            && !running;
                        if primary_btn(
                            ui,
                            if running {
                                "⏳ Building…"
                            } else {
                                "Build ISO"
                            },
                            can_build,
                        ) {
                            do_build = true;
                        }
                        ui.add_space(8.0);
                        if ghost_btn(
                            ui,
                            "Inspect",
                            !self.build.source.trim().is_empty() && !running,
                        ) {
                            do_inspect = true;
                        }
                    });
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        if ghost_btn(ui, "Scan", !running && has_artifact) {
                            do_scan = true;
                        }
                        ui.add_space(4.0);
                        if ghost_btn(ui, "Test Boot", !running && has_artifact) {
                            do_test = true;
                        }
                        ui.add_space(4.0);
                        if ghost_btn(ui, "HTML Report", !running && self.build_result.is_some()) {
                            do_html = true;
                        }
                        ui.add_space(4.0);
                        if ghost_btn(ui, "JSON Report", !running && self.build_result.is_some()) {
                            do_json = true;
                        }
                    });
                });
            });

            // Right: ISO metadata
            ui.vertical(|ui| {
                ui.set_max_width(col_w);
                card(ui, |ui| {
                    ui.label(
                        RichText::new("Detected ISO")
                            .strong()
                            .size(15.0)
                            .color(TEXT),
                    );
                    ui.add_space(8.0);

                    let has_meta = self.inspect_result.is_some() || self.build_result.is_some();
                    if !has_meta {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.label(RichText::new("🔍").size(32.0));
                            ui.add_space(8.0);
                            ui.label(RichText::new("No ISO inspected yet").size(14.0).color(TEXT));
                            ui.label(
                                RichText::new("Enter a source path and click Inspect.")
                                    .size(12.0)
                                    .color(MUTED),
                            );
                        });
                    } else {
                        let distro_str = self
                            .inspect_result
                            .as_ref()
                            .and_then(|m| m.distro.as_ref().map(distro_label))
                            .or_else(|| {
                                self.build_result
                                    .as_ref()
                                    .and_then(|r| r.iso.distro.as_ref().map(distro_label))
                            })
                            .unwrap_or_else(|| "Unknown".into());
                        let release_str = self
                            .inspect_result
                            .as_ref()
                            .and_then(|m| m.release.clone())
                            .or_else(|| {
                                self.build_result
                                    .as_ref()
                                    .and_then(|r| r.iso.release.clone())
                            })
                            .unwrap_or_else(|| "Unknown".into());
                        let arch_str = self
                            .inspect_result
                            .as_ref()
                            .and_then(|m| m.architecture.clone())
                            .or_else(|| {
                                self.build_result
                                    .as_ref()
                                    .and_then(|r| r.iso.architecture.clone())
                            })
                            .unwrap_or_else(|| "Unknown".into());
                        let vol_str = self
                            .inspect_result
                            .as_ref()
                            .and_then(|m| m.volume_id.clone())
                            .or_else(|| {
                                self.build_result
                                    .as_ref()
                                    .and_then(|r| r.iso.volume_id.clone())
                            })
                            .unwrap_or_else(|| "—".into());
                        let sha_str = self
                            .inspect_result
                            .as_ref()
                            .map(|m| format!("{}…", &m.sha256[..20.min(m.sha256.len())]))
                            .or_else(|| {
                                self.build_result.as_ref().map(|r| {
                                    format!("{}…", &r.iso.sha256[..20.min(r.iso.sha256.len())])
                                })
                            })
                            .unwrap_or_else(|| "—".into());
                        let size_str = self
                            .inspect_result
                            .as_ref()
                            .map(|m| fmt_bytes(m.size_bytes))
                            .or_else(|| {
                                self.build_result
                                    .as_ref()
                                    .map(|r| fmt_bytes(r.iso.size_bytes))
                            })
                            .unwrap_or_else(|| "Unknown".into());

                        egui::Grid::new("meta_grid")
                            .num_columns(3)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                for (k, v) in [
                                    ("Distro", &distro_str),
                                    ("Release", &release_str),
                                    ("Architecture", &arch_str),
                                    ("Volume ID", &vol_str),
                                    ("SHA-256", &sha_str),
                                    ("Size", &size_str),
                                ] {
                                    ui.label(RichText::new(k).size(12.0).color(MUTED));
                                    ui.label(RichText::new(v).size(12.0).color(TEXT).monospace());
                                    if k == "SHA-256" {
                                        let full_sha = self
                                            .inspect_result
                                            .as_ref()
                                            .map(|m| m.sha256.clone())
                                            .or_else(|| {
                                                self.build_result
                                                    .as_ref()
                                                    .map(|r| r.iso.sha256.clone())
                                            })
                                            .unwrap_or_default();
                                        if ui
                                            .small_button("📋")
                                            .on_hover_text("Copy full SHA-256")
                                            .clicked()
                                        {
                                            ui.ctx().copy_text(full_sha);
                                        }
                                    } else {
                                        ui.label(""); // keep grid aligned
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                });
            });
        });

        if do_build {
            self.spawn_build();
        }
        if do_inspect {
            self.spawn_inspect();
        }
        if do_scan {
            self.spawn_scan();
        }
        if do_test {
            self.spawn_test_iso();
        }
        if do_html {
            self.spawn_report("html");
        }
        if do_json {
            self.spawn_report("json");
        }

        if let Some(r) = self.build_result.clone() {
            card_green(ui, |ui| {
                ui.label(
                    RichText::new("✓ Build Complete")
                        .size(16.0)
                        .strong()
                        .color(GREEN),
                );
                ui.add_space(8.0);
                for a in &r.artifacts {
                    ui.horizontal(|ui| {
                        ui.label("💿");
                        ui.label(
                            RichText::new(a.to_string_lossy().as_ref())
                                .size(13.0)
                                .monospace()
                                .color(TEXT),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("📋").on_hover_text("Copy path").clicked() {
                                ui.ctx().copy_text(a.to_string_lossy().into_owned());
                            }
                            if ui.small_button("📂").on_hover_text("Open folder").clicked() {
                                if let Some(dir) = a.parent() {
                                    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
                                }
                            }
                        });
                    });
                }
                ui.add_space(10.0);
                if continue_btn(ui, "Continue to Completion →") {
                    self.build_done = true;
                    self.active_stage = Stage::Completion;
                }
            });
        }
    }

    // ── Stage: Completion ─────────────────────────────────────────────────────

    fn show_completion(&mut self, ui: &mut Ui) {
        stage_header(
            ui,
            5,
            "Complete",
            "Pipeline finished. Review artifacts and results below.",
        );

        card(ui, |ui| {
            ui.label(
                RichText::new("Pipeline Summary")
                    .strong()
                    .size(15.0)
                    .color(TEXT),
            );
            ui.add_space(12.0);

            let inj_detail = self.inject_result.as_ref().map(|r| {
                r.artifacts
                    .first()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
            let ver_detail = self.verify_result.as_ref().map(|r| {
                if r.matched {
                    "Checksum matched ✓".into()
                } else {
                    "Checksum mismatch ✗".into()
                }
            });
            let diff_detail = self.diff_result.as_ref().map(|r| {
                format!(
                    "{} added, {} removed, {} modified",
                    r.added.len(),
                    r.removed.len(),
                    r.modified.len()
                )
            });
            let bld_detail = self.build_result.as_ref().map(|r| {
                r.artifacts
                    .first()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });

            for (num, name, done, detail) in [
                (1, "Inject", self.inject_done, &inj_detail),
                (2, "Verify", self.verify_done, &ver_detail),
                (3, "Diff", self.diff_done, &diff_detail),
                (4, "Build", self.build_done, &bld_detail),
            ] {
                let (icon, col) = if done { ("✓", GREEN) } else { ("○", MUTED) };
                Frame::new()
                    .fill(if done {
                        Color32::from_rgb(14, 36, 24)
                    } else {
                        Color32::from_rgb(22, 27, 42)
                    })
                    .stroke(Stroke::new(
                        1.0,
                        if done {
                            Color32::from_rgb(34, 100, 60)
                        } else {
                            CARD_BORDER
                        },
                    ))
                    .inner_margin(12.0f32)
                    .corner_radius(6.0f32)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{icon} Step {num}: {name}"))
                                    .size(14.0)
                                    .strong()
                                    .color(col),
                            );
                            if let Some(d) = detail {
                                if !d.is_empty() {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                RichText::new(d.as_str())
                                                    .size(12.0)
                                                    .color(MUTED)
                                                    .monospace(),
                                            );
                                        },
                                    );
                                }
                            }
                        });
                    });
                ui.add_space(6.0);
            }
        });

        // Artifacts
        let all_artifacts: Vec<String> = self
            .inject_result
            .iter()
            .flat_map(|r| r.artifacts.iter().map(|p| p.to_string_lossy().into_owned()))
            .chain(
                self.build_result
                    .iter()
                    .flat_map(|r| r.artifacts.iter().map(|p| p.to_string_lossy().into_owned())),
            )
            .collect();

        if !all_artifacts.is_empty() {
            card(ui, |ui| {
                ui.label(
                    RichText::new("Output Artifacts")
                        .strong()
                        .size(15.0)
                        .color(TEXT),
                );
                ui.add_space(8.0);
                for path in &all_artifacts {
                    let size_str = std::fs::metadata(path)
                        .map(|m| fmt_bytes(m.len()))
                        .unwrap_or_else(|_| "—".into());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("💿").size(16.0));
                        ui.add_space(6.0);
                        ui.label(RichText::new(path).size(13.0).monospace().color(TEXT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("📋 Copy")
                                .on_hover_text("Copy path to clipboard")
                                .clicked()
                            {
                                ui.ctx().copy_text(path.clone());
                            }
                            if ui.small_button("📂").on_hover_text("Open folder").clicked() {
                                if let Some(dir) = std::path::Path::new(path).parent() {
                                    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
                                }
                            }
                            ui.label(RichText::new(&size_str).size(11.0).color(MUTED));
                        });
                    });
                    ui.add_space(4.0);
                }
            });
        }

        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            let stages = [
                ("← Back to Inject", Stage::Inject),
                ("← Back to Verify", Stage::Verify),
                ("← Back to Diff", Stage::Diff),
                ("← Back to Build", Stage::Build),
            ];
            for (label, target) in stages {
                if ghost_btn(ui, label, true) {
                    self.active_stage = target;
                }
                ui.add_space(8.0);
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ghost_btn(ui, "🔄 New Pipeline", true) {
                self.inject_result = None;
                self.verify_result = None;
                self.iso9660_result = None;
                self.diff_result = None;
                self.build_result = None;
                self.inspect_result = None;
                self.inject_done = false;
                self.verify_done = false;
                self.diff_done = false;
                self.build_done = false;
                self.log_entries.clear();
                self.status = None;
                self.status_since = None;
                self.active_stage = Stage::Inject;
            }
            ui.add_space(8.0);
            if ghost_btn(ui, "🗑 Clear Forms", true) {
                self.inject = InjectState::default();
                self.verify = VerifyState::default();
                self.diff = DiffState::default();
                self.build = BuildState::default();
                self.set_status(StatusMsg::ok("Form state cleared"));
            }
        });
    }
}

// ── eframe::App impl ──────────────────────────────────────────────────────────

impl eframe::App for ForgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_messages(ctx);
        if self.job_running || self.status_since.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        self.render_sidebar(ctx);
        self.render_log_panel(ctx);
        if self.doctor_open {
            self.render_doctor_panel(ctx);
        }
        self.render_main(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let state = PersistedState {
            inject: self.inject.clone(),
            verify: self.verify.clone(),
            diff: self.diff.clone(),
            build: self.build.clone(),
        };
        eframe::set_value(storage, STORAGE_KEY, &state);
    }
}

// ── Build InjectConfig from form state ────────────────────────────────────────

fn build_inject_config(inject: &InjectState) -> InjectConfig {
    let distro = match inject.distro.as_str() {
        "fedora" => Some(Distro::Fedora),
        "arch" => Some(Distro::Arch),
        "mint" => Some(Distro::Mint),
        _ => None,
    };

    InjectConfig {
        source: IsoSource::from_raw(&inject.source),
        out_name: inject.out_name.clone(),
        output_label: opt(&inject.output_label),
        autoinstall_yaml: None,
        hostname: opt(&inject.hostname),
        username: opt(&inject.username),
        password: opt(&inject.password),
        realname: opt(&inject.realname),
        ssh: SshConfig {
            authorized_keys: lines(&inject.ssh_keys),
            allow_password_auth: if inject.ssh_password_auth {
                Some(true)
            } else {
                None
            },
            install_server: if inject.ssh_install_server {
                Some(true)
            } else {
                None
            },
        },
        network: NetworkConfig {
            dns_servers: lines(&inject.dns_servers),
            ntp_servers: lines(&inject.ntp_servers),
        },
        static_ip: opt(&inject.static_ip),
        gateway: opt(&inject.gateway),
        proxy: ProxyConfig {
            http_proxy: opt(&inject.http_proxy),
            https_proxy: opt(&inject.https_proxy),
            no_proxy: Vec::new(),
        },
        timezone: opt(&inject.timezone),
        locale: opt(&inject.locale),
        keyboard_layout: opt(&inject.keyboard_layout),
        storage_layout: opt(&inject.storage_layout),
        apt_mirror: opt(&inject.apt_mirror),
        extra_packages: lines(&inject.packages),
        wallpaper: opt(&inject.wallpaper_path).map(PathBuf::from),
        extra_late_commands: lines(&inject.late_commands),
        no_user_interaction: inject.no_user_interaction,
        user: UserConfig::default(),
        firewall: FirewallConfig {
            enabled: inject.firewall_enabled,
            default_policy: opt(&inject.firewall_policy),
            allow_ports: lines(&inject.allow_ports),
            deny_ports: lines(&inject.deny_ports),
        },
        enable_services: Vec::new(),
        disable_services: Vec::new(),
        sysctl: Vec::new(),
        swap: inject
            .swap_size_mb
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|&n| n > 0)
            .map(|size_mb| SwapConfig {
                size_mb,
                filename: None,
                swappiness: None,
            }),
        apt_repos: lines(&inject.apt_repos),
        containers: ContainerConfig {
            docker: inject.docker,
            podman: inject.podman,
            docker_users: Vec::new(),
        },
        grub: GrubConfig::default(),
        encrypt: false,
        encrypt_passphrase: None,
        mounts: Vec::new(),
        run_commands: lines(&inject.run_commands),
        distro,
    }
}
