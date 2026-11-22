#![forbid(unsafe_code)]
//! Tabbed breakdown table panel (`PAGES`, `SOURCES`, `GEO`, `TECH`, `EVENTS`).

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Row, Table};

use crate::fmt::format_count;
use crate::state::{AppState, BreakdownTab};

/// Renders the tabbed breakdown panel and selectable rows with visitor share percentages.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn render_breakdown(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let mut tab_spans = vec![Span::styled(
        " BREAKDOWN [Tab]: ",
        Style::default().fg(Color::DarkGray),
    )];
    for (idx, t) in BreakdownTab::ALL.iter().enumerate() {
        if idx > 0 {
            tab_spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
        }
        let active = *t == app.tab;
        let text = if active {
            format!("*{}*", t.label())
        } else {
            t.label().to_string()
        };
        let style = if active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        tab_spans.push(Span::styled(text, style));
    }

    let total_visitors = app.query.as_ref().map_or(1, |q| q.summary.visitors.max(1));
    let key_name = app.tab.row_key();
    let rows: Vec<Row<'static>> = app.query.as_ref().map_or_else(Vec::new, |q| {
        q.rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let mark = if i == app.selected_row { ">" } else { " " };
                let name = row.key.get(key_name).map_or("(none)", String::as_str);
                let share = (row.visitors as f64 / total_visitors as f64) * 100.0;
                let style = if i == app.selected_row {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Row::new(vec![
                    format!("{mark} {name}"),
                    format!("{share:>5.1}%"),
                    format!("{:>10}", format_count(row.visitors)),
                ])
                .style(style)
            })
            .collect()
    });

    let header = Row::new(vec!["ITEM", "SHARE", "VISITORS"]).style(
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    let table = Table::new(
        rows,
        [
            Constraint::Min(24),
            Constraint::Length(8),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Line::from(tab_spans)),
    );
    frame.render_widget(table, area);
}
