#![forbid(unsafe_code)]
//! Headless `ratatui::TestBackend` rendering and keyboard interaction tests for `rotameter-tui`.

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use rotameter_client::types::{
    BreakdownRow, KpiSummary, QueryMeta, QueryResponse, QueryWindow, RangePreset, RealtimePage,
    RealtimePulsePoint, RealtimeResponse, SiteId, SiteRef, TimeseriesRow,
};
use rotameter_tui::input::{KeyAction, handle_key_event};
use rotameter_tui::render::draw_ui;
use rotameter_tui::state::{AppState, BreakdownTab};

fn b_row(path: &str, visitors: u64, pageviews: u64, visits: u64, dur: u64) -> BreakdownRow {
    let mut key = BTreeMap::new();
    key.insert("path".into(), path.into());
    BreakdownRow {
        key,
        visitors,
        pageviews,
        visits,
        bounce_rate: 0.35,
        avg_visit_duration_ms: dur,
    }
}

fn ts_row(bucket: &str, visitors: u64, pageviews: u64, visits: u64) -> TimeseriesRow {
    TimeseriesRow {
        bucket: bucket.into(),
        visitors,
        pageviews,
        visits,
    }
}

fn sample_query() -> QueryResponse {
    QueryResponse {
        site_id: SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10]),
        slug: "colinwolff.dev".into(),
        timezone: "UTC".into(),
        window: QueryWindow {
            from: "2026-10-24T00:00:00Z".into(),
            to: "2026-11-23T00:00:00Z".into(),
        },
        summary: KpiSummary {
            visitors: 1_842,
            pageviews: 4_190,
            visits: 2_110,
            views_per_visit: 1.99,
            bounce_rate: 0.482,
            avg_visit_duration_ms: 94_000,
        },
        previous_summary: Some(KpiSummary {
            visitors: 1_510,
            pageviews: 3_300,
            visits: 1_700,
            views_per_visit: 1.94,
            bounce_rate: 0.497,
            avg_visit_duration_ms: 88_000,
        }),
        timeseries: vec![
            ts_row("2026-11-18T00:00:00Z", 90, 190, 100),
            ts_row("2026-11-19T00:00:00Z", 112, 248, 120),
            ts_row("2026-11-20T00:00:00Z", 150, 340, 165),
        ],
        rows: vec![
            b_row("/projects/rotameter", 412, 920, 440, 110_000),
            b_row("/blog/edge-ssr", 297, 640, 310, 85_000),
        ],
        meta: QueryMeta {
            total_rows: 2,
            query_ms: 1.8,
            truncated_by_cardinality_cap: false,
        },
    }
}

fn sample_realtime() -> RealtimeResponse {
    RealtimeResponse {
        site_id: SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10]),
        slug: "colinwolff.dev".into(),
        active_visitors: 3,
        active_pages: vec![RealtimePage {
            path: "/".into(),
            active_visitors: 3,
        }],
        pulse: vec![RealtimePulsePoint {
            minute: "2026-11-22T12:10:00Z".into(),
            events: 5,
        }],
    }
}

fn render_text(app: &AppState) -> String {
    let backend = TestBackend::new(100, 26);
    if let Ok(mut term) = Terminal::new(backend)
        && term.draw(|f| draw_ui(f, app)).is_ok()
    {
        let buf = term.backend().buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        return out;
    }
    String::new()
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn flight_instrument_renders_kpis_chart_scrub_tabs_filters_and_modals() {
    let mut app = AppState::new(
        SiteRef::Slug("colinwolff.dev".into()),
        vec!["colinwolff.dev".into(), "demo.local".into()],
    );
    app.apply_snapshot(sample_query(), sample_realtime());

    let screen = render_text(&app);
    assert!(screen.contains("ROTAMETER // colinwolff.dev") && screen.contains("● 3 LIVE"));
    assert!(screen.contains("1,842") && screen.contains("+22%") && screen.contains("01m34s"));
    assert!(screen.contains("/projects/rotameter") && screen.contains("LATEST: 2026.11.20"));

    assert_eq!(
        handle_key_event(&mut app, key(KeyCode::Left)),
        KeyAction::None
    );
    assert!(render_text(&app).contains("CURSOR: 2026.11.19 · 112 VISITORS · 248 VIEWS"));

    assert_eq!(
        handle_key_event(&mut app, key(KeyCode::Down)),
        KeyAction::None
    );
    assert_eq!(
        handle_key_event(&mut app, key(KeyCode::Enter)),
        KeyAction::Refresh
    );
    assert_eq!(app.filters.len(), 1);
    assert!(render_text(&app).contains("[path=/blog/edge-ssr]"));
    assert_eq!(
        handle_key_event(&mut app, key(KeyCode::Esc)),
        KeyAction::Refresh
    );
    assert_eq!(app.filters.len(), 0);

    assert_eq!(
        handle_key_event(&mut app, key(KeyCode::Tab)),
        KeyAction::Refresh
    );
    assert_eq!(app.tab, BreakdownTab::Sources);
    assert_eq!(
        handle_key_event(&mut app, key(KeyCode::BackTab)),
        KeyAction::Refresh
    );
    assert_eq!(app.tab, BreakdownTab::Pages);
    assert_eq!(
        handle_key_event(&mut app, key(KeyCode::Char('1'))),
        KeyAction::Refresh
    );
    assert_eq!(app.range, RangePreset::Hours24);

    assert_eq!(
        handle_key_event(&mut app, key(KeyCode::Char('s'))),
        KeyAction::None
    );
    assert!(render_text(&app).contains("SWITCH SITE"));
    assert_eq!(
        handle_key_event(&mut app, key(KeyCode::Down)),
        KeyAction::None
    );
    assert_eq!(
        handle_key_event(&mut app, key(KeyCode::Enter)),
        KeyAction::Refresh
    );
    assert_eq!(app.active_site.to_string(), "demo.local");

    assert_eq!(
        handle_key_event(&mut app, key(KeyCode::Char('?'))),
        KeyAction::None
    );
    assert!(render_text(&app).contains("KEYBINDINGS"));
    assert_eq!(
        handle_key_event(&mut app, key(KeyCode::Char('q'))),
        KeyAction::Quit
    );
}
