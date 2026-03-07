use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use forgeiso_engine::{BuildConfig, ForgeIsoEngine, IsoSource, ProfileKind};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Row, Table},
    Terminal,
};
use tokio::sync::mpsc;

// Messages sent from background tasks back to the UI loop.
enum WorkerMsg {
    InspectOk(Box<forgeiso_engine::IsoMetadata>),
    BuildOk(Box<forgeiso_engine::BuildResult>),
    ScanOk(String),
    TestOk(bool),
    OpError(String),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Restore terminal on panic so the user doesn't get a broken shell.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let engine = Arc::new(ForgeIsoEngine::new());
    let mut app = App::new(engine.doctor().await);
    let mut rx_events = engine.subscribe();

    let (tx, mut rx_worker) = mpsc::unbounded_channel::<WorkerMsg>();

    loop {
        // Drain engine broadcast events (progress logs).
        while let Ok(ev) = rx_events.try_recv() {
            app.push_log(format!("[{:?}] {}", ev.phase, ev.message));
        }

        // Drain worker results.
        while let Ok(msg) = rx_worker.try_recv() {
            app.busy = false;
            match msg {
                WorkerMsg::InspectOk(info) => {
                    app.inspection = vec![
                        format!("Source: {}", info.source_path.display()),
                        format!(
                            "Distro: {}",
                            info.distro
                                .map(|d| format!("{d:?}"))
                                .unwrap_or_else(|| "unknown".into())
                        ),
                        format!("Release: {}", info.release.as_deref().unwrap_or("unknown")),
                        format!(
                            "Arch: {}",
                            info.architecture.as_deref().unwrap_or("unknown")
                        ),
                        format!("Volume: {}", info.volume_id.as_deref().unwrap_or("unknown")),
                    ];
                    app.status = "Inspection completed".into();
                }
                WorkerMsg::BuildOk(result) => {
                    app.last_iso = result.artifacts.first().cloned();
                    let label = result
                        .artifacts
                        .first()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| result.output_dir.display().to_string());
                    app.inspection = vec![
                        format!("Built ISO: {label}"),
                        format!("Report JSON: {}", result.report_json.display()),
                        format!("Report HTML: {}", result.report_html.display()),
                    ];
                    app.status = format!("Build completed: {label}");
                }
                WorkerMsg::ScanOk(path) => {
                    app.status = format!("Scan completed: {path}");
                }
                WorkerMsg::TestOk(passed) => {
                    app.status = format!("Test completed: passed={passed}");
                }
                WorkerMsg::OpError(e) => {
                    app.status = format!("Error: {e}");
                }
            }
        }

        terminal.draw(|f| ui(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if app.editing {
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter => app.editing = false,
                        KeyCode::Backspace => app.backspace(),
                        KeyCode::Char(ch) => app.push_char(ch),
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Up => app.previous_field(),
                    KeyCode::Down => app.next_field(),
                    KeyCode::Enter => app.editing = true,
                    KeyCode::Char('i') if !app.busy => {
                        app.spawn_inspect(Arc::clone(&engine), tx.clone());
                    }
                    KeyCode::Char('b') if !app.busy => {
                        app.spawn_build(Arc::clone(&engine), tx.clone());
                    }
                    KeyCode::Char('s') if !app.busy => {
                        app.spawn_scan(Arc::clone(&engine), tx.clone());
                    }
                    KeyCode::Char('t') if !app.busy => {
                        app.spawn_test(Arc::clone(&engine), tx.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

struct App {
    selected_field: usize,
    editing: bool,
    busy: bool,
    source: String,
    output_dir: String,
    build_name: String,
    overlay_dir: String,
    profile: String,
    inspection: Vec<String>,
    logs: Vec<String>,
    status: String,
    last_iso: Option<PathBuf>,
}

impl App {
    fn new(doctor: forgeiso_engine::DoctorReport) -> Self {
        let mut logs = Vec::new();
        logs.push(format!(
            "doctor: host={} arch={} linux_supported={}",
            doctor.host_os, doctor.host_arch, doctor.linux_supported
        ));
        for (tool, available) in doctor.tooling {
            logs.push(format!("tool {tool}: {available}"));
        }

        Self {
            selected_field: 0,
            editing: false,
            busy: false,
            source: String::new(),
            output_dir: "/tmp/forgeoutput".to_string(),
            build_name: "forgeiso-local".to_string(),
            overlay_dir: String::new(),
            profile: "minimal".to_string(),
            inspection: vec!["No ISO inspected yet".to_string()],
            logs,
            status: "Ready".to_string(),
            last_iso: None,
        }
    }

    fn fields(&self) -> [(&str, &str); 5] {
        [
            ("Source", &self.source),
            ("Output", &self.output_dir),
            ("Name", &self.build_name),
            ("Overlay", &self.overlay_dir),
            ("Profile", &self.profile),
        ]
    }

    fn next_field(&mut self) {
        self.selected_field = (self.selected_field + 1) % self.fields().len();
    }

    fn previous_field(&mut self) {
        if self.selected_field == 0 {
            self.selected_field = self.fields().len() - 1;
        } else {
            self.selected_field -= 1;
        }
    }

    fn current_mut(&mut self) -> &mut String {
        match self.selected_field {
            0 => &mut self.source,
            1 => &mut self.output_dir,
            2 => &mut self.build_name,
            3 => &mut self.overlay_dir,
            _ => &mut self.profile,
        }
    }

    fn push_char(&mut self, ch: char) {
        self.current_mut().push(ch);
    }

    fn backspace(&mut self) {
        self.current_mut().pop();
    }

    fn push_log(&mut self, line: String) {
        self.logs.push(line);
        if self.logs.len() > 12 {
            self.logs.remove(0);
        }
    }

    fn spawn_inspect(&mut self, engine: Arc<ForgeIsoEngine>, tx: mpsc::UnboundedSender<WorkerMsg>) {
        if self.source.trim().is_empty() {
            self.status = "Source is required".into();
            return;
        }
        self.busy = true;
        self.status = "Inspecting…".into();
        let source = self.source.clone();
        tokio::spawn(async move {
            let msg = match engine.inspect_source(&source, None).await {
                Ok(info) => WorkerMsg::InspectOk(Box::new(info)),
                Err(e) => WorkerMsg::OpError(format!("Inspect failed: {e}")),
            };
            let _ = tx.send(msg);
        });
    }

    fn spawn_build(&mut self, engine: Arc<ForgeIsoEngine>, tx: mpsc::UnboundedSender<WorkerMsg>) {
        let cfg = match self.build_config() {
            Ok(cfg) => cfg,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        self.busy = true;
        self.status = "Building…".into();
        let out_dir = PathBuf::from(&self.output_dir);
        tokio::spawn(async move {
            let msg = match engine.build(&cfg, &out_dir).await {
                Ok(r) => WorkerMsg::BuildOk(Box::new(r)),
                Err(e) => WorkerMsg::OpError(format!("Build failed: {e}")),
            };
            let _ = tx.send(msg);
        });
    }

    fn spawn_scan(&mut self, engine: Arc<ForgeIsoEngine>, tx: mpsc::UnboundedSender<WorkerMsg>) {
        let Some(artifact) = self.last_iso.clone() else {
            self.status = "Build an ISO before running scan".into();
            return;
        };
        self.busy = true;
        self.status = "Scanning…".into();
        let out = artifact
            .parent()
            .map(|p| p.join("scan"))
            .unwrap_or_else(|| PathBuf::from("scan"));
        tokio::spawn(async move {
            let msg = match engine.scan(&artifact, None, &out).await {
                Ok(r) => WorkerMsg::ScanOk(r.report_json.display().to_string()),
                Err(e) => WorkerMsg::OpError(format!("Scan failed: {e}")),
            };
            let _ = tx.send(msg);
        });
    }

    fn spawn_test(&mut self, engine: Arc<ForgeIsoEngine>, tx: mpsc::UnboundedSender<WorkerMsg>) {
        let Some(artifact) = self.last_iso.clone() else {
            self.status = "Build an ISO before running tests".into();
            return;
        };
        self.busy = true;
        self.status = "Testing…".into();
        let out = artifact
            .parent()
            .map(|p| p.join("test"))
            .unwrap_or_else(|| PathBuf::from("test"));
        tokio::spawn(async move {
            let msg = match engine.test_iso(&artifact, true, true, &out).await {
                Ok(r) => WorkerMsg::TestOk(r.passed),
                Err(e) => WorkerMsg::OpError(format!("Test failed: {e}")),
            };
            let _ = tx.send(msg);
        });
    }

    fn build_config(&self) -> Result<BuildConfig, String> {
        if self.source.trim().is_empty() {
            return Err("Source is required".to_string());
        }

        let overlay_dir = if self.overlay_dir.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(self.overlay_dir.trim()))
        };

        let profile = match self.profile.trim() {
            "minimal" => ProfileKind::Minimal,
            "desktop" => ProfileKind::Desktop,
            other => return Err(format!("Unsupported profile '{other}'")),
        };

        Ok(BuildConfig {
            name: self.build_name.clone(),
            source: IsoSource::from_raw(self.source.clone()),
            overlay_dir,
            output_label: None,
            profile,
            auto_scan: false,
            auto_test: false,
            scanning: Default::default(),
            testing: Default::default(),
            keep_workdir: false,
        })
    }
}

fn ui(frame: &mut ratatui::Frame<'_>, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let rows = app
        .fields()
        .iter()
        .enumerate()
        .map(|(idx, (label, value))| {
            let style = if idx == app.selected_field {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            Row::new(vec![(*label).to_string(), (*value).to_string()]).style(style)
        })
        .collect::<Vec<_>>();

    let form_title = if app.busy {
        "Local Build Form [busy…]"
    } else {
        "Local Build Form"
    };
    let table = Table::new(rows, [Constraint::Length(10), Constraint::Min(30)])
        .block(Block::default().borders(Borders::ALL).title(form_title))
        .row_highlight_style(Style::default().bold());
    frame.render_widget(table, chunks[0]);

    let inspect_lines = app
        .inspection
        .iter()
        .map(|line| Line::from(line.clone()))
        .collect::<Vec<_>>();
    let inspect = Paragraph::new(inspect_lines)
        .block(Block::default().borders(Borders::ALL).title("Inspection"));
    frame.render_widget(inspect, chunks[1]);

    let log_lines = app
        .logs
        .iter()
        .map(|line| Line::from(line.clone()))
        .collect::<Vec<_>>();
    let logs =
        Paragraph::new(log_lines).block(Block::default().borders(Borders::ALL).title("Logs"));
    frame.render_widget(logs, chunks[2]);

    let help = Paragraph::new(vec![Line::from(
        "Up/Down select, Enter edit, i inspect, b build, s scan, t test, q quit",
    )])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(app.status.as_str()),
    );
    frame.render_widget(help, chunks[3]);

    if app.editing {
        let popup = centered_rect(60, 18, frame.area());
        frame.render_widget(Clear, popup);
        let edit =
            Paragraph::new("Typing into selected field. Press Enter or Esc to stop editing.")
                .block(Block::default().borders(Borders::ALL).title("Editing"));
        frame.render_widget(edit, popup);
    }
}

fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    area: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
