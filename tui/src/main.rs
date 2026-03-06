use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use forgeiso_engine::ForgeIsoEngine;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Terminal,
};

const TAB_TITLES: [&str; 5] = ["Configure", "Build", "Scans", "Tests", "Reports"];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let engine = ForgeIsoEngine::new();
    let doctor = engine.doctor().await;
    app.logs.push(format!(
        "doctor completed at {} (docker={}, podman={})",
        doctor.timestamp,
        doctor.runtime_candidates.get("docker").copied().unwrap_or(false),
        doctor.runtime_candidates.get("podman").copied().unwrap_or(false)
    ));

    let mut rx = engine.subscribe();
    let mut done = false;

    while !done {
        while let Ok(event) = rx.try_recv() {
            app.logs
                .push(format!("[{:?}] {}", event.phase, event.message));
            if app.logs.len() > 200 {
                app.logs.remove(0);
            }
        }

        terminal.draw(|f| ui(f, &app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => done = true,
                    KeyCode::Right => app.next_tab(),
                    KeyCode::Left => app.previous_tab(),
                    KeyCode::Char('d') => {
                        app.logs
                            .push("doctor command already executed in this session".to_string());
                    }
                    KeyCode::Char('c') => app.clear_logs(),
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

#[derive(Default)]
struct App {
    tab_index: usize,
    logs: Vec<String>,
}

impl App {
    fn new() -> Self {
        Self {
            tab_index: 0,
            logs: vec![
                "ForgeISO TUI initialized".to_string(),
                "Shortcuts: Left/Right switch tabs, d doctor, c clear logs, q quit".to_string(),
            ],
        }
    }

    fn next_tab(&mut self) {
        self.tab_index = (self.tab_index + 1) % TAB_TITLES.len();
    }

    fn previous_tab(&mut self) {
        if self.tab_index == 0 {
            self.tab_index = TAB_TITLES.len() - 1;
        } else {
            self.tab_index -= 1;
        }
    }

    fn clear_logs(&mut self) {
        self.logs.clear();
        self.logs.push("logs cleared".to_string());
    }
}

fn ui(frame: &mut ratatui::Frame<'_>, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(10),
        ])
        .split(frame.area());

    render_tabs(frame, chunks[0], app);
    render_main(frame, chunks[1], app);
    render_logs(frame, chunks[2], app);
}

fn render_tabs(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let titles = TAB_TITLES
        .iter()
        .map(|t| Line::from(Span::raw(*t)))
        .collect::<Vec<_>>();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("ForgeISO"))
        .select(app.tab_index)
        .highlight_style(Style::default().fg(Color::Cyan).bold())
        .divider(Span::raw(" | "));

    frame.render_widget(tabs, area);
}

fn render_main(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let text = match app.tab_index {
        0 => "Configure\n- Distro policy and release validation\n- Profile presets\n- User/SSH hardening",
        1 => "Build\n- Containerized backend execution\n- Live logs from engine events\n- Artifact generation",
        2 => "Scans\n- SBOM generation\n- Vulnerability/compliance/secrets gates",
        3 => "Tests\n- BIOS/UEFI smoke checks\n- Serial log and screenshot collection",
        _ => "Reports\n- JSON and HTML export\n- Build provenance and security summaries",
    };

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Panel"));

    frame.render_widget(paragraph, area);
}

fn render_logs(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let lines = app
        .logs
        .iter()
        .rev()
        .take(8)
        .rev()
        .map(|line| Line::from(line.clone()))
        .collect::<Vec<_>>();

    let logs = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Live Logs"));

    frame.render_widget(logs, area);
}
