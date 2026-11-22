#![forbid(unsafe_code)]
//! Main `ratatui` flight-instrument frame layout, header bar, KPI strip, and footer.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use rotameter_client::types::RangePreset;

use crate::chart::render_chart;
use crate::fmt::{format_count, format_delta, format_duration};
use crate::modals::{render_help_modal, render_site_switcher};
use crate::panels::render_breakdown;
use crate::state::AppState;

/// Renders the entire Rotameter TUI frame onto `frame`.
pub fn draw_ui(frame: &mut Frame<'_>, app: &AppState) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Min(6),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_header(frame, root[0], app);

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(32), Constraint::Min(30)])
        .split(root[1]);
    render_kpis(frame, mid[0], app);
    render_chart(frame, mid[1], app);

    render_breakdown(frame, root[2], app);
    render_footer(frame, root[3], app);

    if app.site_switcher_open {
        render_site_switcher(frame, app);
    } else if app.help_open {
        render_help_modal(frame);
    }
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let presets = [
        (RangePreset::Hours24, "1:24H"),
        (RangePreset::Days7, "2:7D"),
        (RangePreset::Days30, "3:30D"),
        (RangePreset::Months12, "4:12M"),
        (RangePreset::All, "5:ALL"),
    ];
    let mut spans = vec![Span::styled(
        format!(" ROTAMETER // {}   ", app.active_site),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];
    for (preset, tag) in presets {
        let active = app.range == preset;
        let text = if active {
            format!("[*{tag}*] ")
        } else {
            format!("[{tag}] ")
        };
        let style = if active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        spans.push(Span::styled(text, style));
    }
    let live = app.realtime.as_ref().map_or(0, |r| r.active_visitors);
    let sync = if app.loading { " · SYNC..." } else { "" };
    spans.push(Span::styled(
        format!("  ● {live} LIVE{sync}"),
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

#[allow(clippy::cast_precision_loss)]
fn render_kpis(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lines = if let Some(q) = &app.query {
        let s = &q.summary;
        let p = q.previous_summary.as_ref();
        vec![
            kpi_line(
                "VISITORS",
                &format_count(s.visitors),
                p.map(|pv| format_delta(s.visitors as f64, pv.visitors as f64)),
            ),
            kpi_line(
                "PAGEVIEWS",
                &format_count(s.pageviews),
                p.map(|pv| format_delta(s.pageviews as f64, pv.pageviews as f64)),
            ),
            kpi_line(
                "VISITS",
                &format_count(s.visits),
                p.map(|pv| format_delta(s.visits as f64, pv.visits as f64)),
            ),
            kpi_line(
                "VIEWS/VISIT",
                &format!("{:.2}", s.views_per_visit),
                p.map(|pv| format_delta(s.views_per_visit, pv.views_per_visit)),
            ),
            kpi_line(
                "BOUNCE RATE",
                &format!("{:.1}%", s.bounce_rate * 100.0),
                p.map(|pv| format_delta(s.bounce_rate, pv.bounce_rate)),
            ),
            kpi_line(
                "AVG DURATION",
                &format_duration(s.avg_visit_duration_ms),
                p.map(|pv| {
                    format_delta(
                        s.avg_visit_duration_ms as f64,
                        pv.avg_visit_duration_ms as f64,
                    )
                }),
            ),
        ]
    } else {
        vec![Line::from("loading metrics...")]
    };
    frame.render_widget(Paragraph::new(lines), area);
}

fn kpi_line(label: &str, value: &str, delta: Option<String>) -> Line<'static> {
    let d = delta.unwrap_or_else(|| "--".to_string());
    let d_color = if d.starts_with('+') {
        Color::Green
    } else if d.starts_with('-') && d != "--" {
        Color::Red
    } else {
        Color::DarkGray
    };
    Line::from(vec![
        Span::styled(
            format!(" {label:<12}"),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{value:>10} "),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{d:>6}"), Style::default().fg(d_color)),
    ])
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let msg = app.status_msg.as_deref().unwrap_or(
        " [s] site  [1-5] range  [←/→] scrub bar  [Tab] panel  [Enter] filter  [Esc] clear  [?] help  [q] quit",
    );
    let color = if app.status_msg.is_some() {
        Color::Red
    } else {
        Color::DarkGray
    };
    frame.render_widget(
        Paragraph::new(Span::styled(msg, Style::default().fg(color))),
        area,
    );
}
