#![forbid(unsafe_code)]
//! Keyboard event dispatcher for range hotkeys, scrubbing, breakdown drill-down, and overlays.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use rotameter_client::types::{FilterOp, FilterValue, QueryFilter, RangePreset, SiteRef};

use crate::state::AppState;

/// Action requested by handling a single terminal key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// UI-only state change; no network refresh needed.
    None,
    /// State changed requiring a background query refresh.
    Refresh,
    /// User requested clean exit.
    Quit,
}

/// Applies one terminal key press to [`AppState`] and returns the resulting [`KeyAction`].
pub fn handle_key_event(app: &mut AppState, key: KeyEvent) -> KeyAction {
    if key.kind != KeyEventKind::Press {
        return KeyAction::None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return KeyAction::Quit;
    }
    if key.code == KeyCode::Char('q') {
        app.should_quit = true;
        return KeyAction::Quit;
    }
    if key.code == KeyCode::Char('?') {
        app.help_open = !app.help_open;
        app.site_switcher_open = false;
        return KeyAction::None;
    }
    if app.help_open && matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
        app.help_open = false;
        return KeyAction::None;
    }
    if app.site_switcher_open {
        return handle_site_switcher_key(app, key.code);
    }
    handle_dashboard_key(app, key.code)
}

fn handle_site_switcher_key(app: &mut AppState, code: KeyCode) -> KeyAction {
    let count = app.sites.len();
    match code {
        KeyCode::Esc | KeyCode::Char('s') => app.site_switcher_open = false,
        KeyCode::Up | KeyCode::Char('k') if app.site_picker_idx > 0 => app.site_picker_idx -= 1,
        KeyCode::Down | KeyCode::Char('j') if count > 0 && app.site_picker_idx + 1 < count => {
            app.site_picker_idx += 1;
        }
        KeyCode::Enter if count > 0 => {
            if let Some(slug) = app.sites.get(app.site_picker_idx).cloned() {
                app.active_site = SiteRef::Slug(slug);
                app.site_switcher_open = false;
                app.filters.clear();
                app.scrub_idx = None;
                app.selected_row = 0;
                app.loading = true;
                return KeyAction::Refresh;
            }
        }
        _ => {}
    }
    KeyAction::None
}

fn handle_dashboard_key(app: &mut AppState, code: KeyCode) -> KeyAction {
    match code {
        KeyCode::Char('1') => return set_range(app, RangePreset::Hours24),
        KeyCode::Char('2') => return set_range(app, RangePreset::Days7),
        KeyCode::Char('3') => return set_range(app, RangePreset::Days30),
        KeyCode::Char('4') => return set_range(app, RangePreset::Months12),
        KeyCode::Char('5') => return set_range(app, RangePreset::All),
        KeyCode::Left | KeyCode::Char('h') => scrub_horizontal(app, -1),
        KeyCode::Right | KeyCode::Char('l') => scrub_horizontal(app, 1),
        KeyCode::Up | KeyCode::Char('k') if app.selected_row > 0 => app.selected_row -= 1,
        KeyCode::Down | KeyCode::Char('j') => move_row_down(app),
        KeyCode::Tab => {
            app.tab = app.tab.next();
            app.selected_row = 0;
            app.loading = true;
            return KeyAction::Refresh;
        }
        KeyCode::BackTab => {
            app.tab = app.tab.prev();
            app.selected_row = 0;
            app.loading = true;
            return KeyAction::Refresh;
        }
        KeyCode::Enter => return push_row_filter(app),
        KeyCode::Esc => {
            if app.filters.pop().is_some() {
                app.selected_row = 0;
                app.loading = true;
                return KeyAction::Refresh;
            }
            app.scrub_idx = None;
        }
        KeyCode::Char('s') => open_site_switcher(app),
        KeyCode::Char('r') => {
            app.loading = true;
            return KeyAction::Refresh;
        }
        _ => {}
    }
    KeyAction::None
}

fn set_range(app: &mut AppState, range: RangePreset) -> KeyAction {
    app.range = range;
    app.scrub_idx = None;
    app.selected_row = 0;
    app.loading = true;
    KeyAction::Refresh
}

fn scrub_horizontal(app: &mut AppState, delta: isize) {
    let len = app.query.as_ref().map_or(0, |q| q.timeseries.len());
    if len == 0 {
        return;
    }
    let next = match app.scrub_idx {
        None => {
            if delta < 0 {
                len.saturating_sub(2).min(len - 1)
            } else {
                len - 1
            }
        }
        Some(cur) => {
            if delta < 0 {
                cur.saturating_sub(1)
            } else {
                (cur + 1).min(len - 1)
            }
        }
    };
    app.scrub_idx = Some(next);
}

fn move_row_down(app: &mut AppState) {
    let rows = app.query.as_ref().map_or(0, |q| q.rows.len());
    if rows > 0 && app.selected_row + 1 < rows {
        app.selected_row += 1;
    }
}

fn push_row_filter(app: &mut AppState) -> KeyAction {
    let Some(q) = &app.query else {
        return KeyAction::None;
    };
    let Some(row) = q.rows.get(app.selected_row) else {
        return KeyAction::None;
    };
    let Some(val) = row.key.get(app.tab.row_key()) else {
        return KeyAction::None;
    };
    let filter = QueryFilter {
        dimension: app.tab.dimension(),
        op: FilterOp::Eq,
        value: FilterValue::Scalar(val.clone()),
    };
    if !app.filters.contains(&filter) {
        app.filters.push(filter);
    }
    app.selected_row = 0;
    app.loading = true;
    KeyAction::Refresh
}

fn open_site_switcher(app: &mut AppState) {
    let current = app.active_site.to_string();
    app.site_picker_idx = app.sites.iter().position(|s| *s == current).unwrap_or(0);
    app.site_switcher_open = true;
}
