#![forbid(unsafe_code)]
//! Terminal setup/teardown RAII guard and non-blocking `30s` polling event loop.

use std::io::{self, Stdout};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use rotameter_client::types::{QueryRequest, QueryResponse, RealtimeResponse, SiteRef};

use crate::TuiError;
use crate::input::{KeyAction, handle_key_event};
use crate::render::draw_ui;
use crate::state::AppState;

const POLL_INTERVAL: Duration = Duration::from_secs(30);
const EVENT_TICK: Duration = Duration::from_millis(100);

/// Blocking API adapter executed on the background worker thread.
pub(crate) trait TuiBackend: Send {
    /// Lists accessible site slugs, if allowed by token scope.
    fn list_site_slugs(&self) -> Vec<String>;
    /// Fetches query KPIs, time series, and breakdown rows.
    fn fetch_query(&self, req: &QueryRequest) -> Result<QueryResponse, String>;
    /// Fetches trailing realtime active visitors and 30m pulse.
    fn fetch_realtime(&self, site: &SiteRef) -> Result<RealtimeResponse, String>;
    /// Default target site configured on the profile.
    fn default_site(&self) -> Option<String>;
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn enter() -> Result<Self, TuiError> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

struct FetchResult {
    sites: Option<Vec<String>>,
    outcome: Result<(QueryResponse, RealtimeResponse), String>,
}

/// Runs the interactive terminal dashboard until user exit.
pub(crate) fn run_event_loop<B: TuiBackend + 'static>(backend: B) -> Result<(), TuiError> {
    let default_slug = backend
        .default_site()
        .unwrap_or_else(|| "default".to_string());
    let initial_site = default_slug
        .parse::<SiteRef>()
        .unwrap_or_else(|_| SiteRef::Slug(default_slug.clone()));
    let mut app = AppState::new(initial_site, vec![default_slug]);
    let mut guard = TerminalGuard::enter()?;

    let (req_tx, req_rx) = mpsc::channel::<(QueryRequest, SiteRef, bool)>();
    let (res_tx, res_rx) = mpsc::channel::<FetchResult>();
    let _worker = thread::spawn(move || {
        while let Ok((req, site, fetch_sites)) = req_rx.recv() {
            let sites = fetch_sites.then(|| backend.list_site_slugs());
            let outcome = match (backend.fetch_query(&req), backend.fetch_realtime(&site)) {
                (Ok(q), Ok(rt)) => Ok((q, rt)),
                (Err(e), _) | (_, Err(e)) => Err(e),
            };
            if res_tx.send(FetchResult { sites, outcome }).is_err() {
                break;
            }
        }
    });

    let _ = req_tx.send((app.build_query_request(), app.active_site.clone(), true));
    let mut last_poll = Instant::now();

    while !app.should_quit {
        while let Ok(msg) = res_rx.try_recv() {
            if let Some(sites) = msg.sites
                && !sites.is_empty()
            {
                app.sites = sites;
            }
            match msg.outcome {
                Ok((q, rt)) => app.apply_snapshot(q, rt),
                Err(err) => {
                    app.loading = false;
                    app.status_msg = Some(err);
                }
            }
        }

        guard.terminal.draw(|f| draw_ui(f, &app))?;

        if event::poll(EVENT_TICK)?
            && let Event::Key(key) = event::read()?
        {
            match handle_key_event(&mut app, key) {
                KeyAction::Quit => break,
                KeyAction::Refresh => {
                    last_poll = Instant::now();
                    let _ =
                        req_tx.send((app.build_query_request(), app.active_site.clone(), false));
                }
                KeyAction::None => {}
            }
        }

        if last_poll.elapsed() >= POLL_INTERVAL && !app.loading {
            last_poll = Instant::now();
            app.loading = true;
            let _ = req_tx.send((app.build_query_request(), app.active_site.clone(), false));
        }
    }

    Ok(())
}
