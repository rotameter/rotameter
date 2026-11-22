#![forbid(unsafe_code)]
//! ASCII block chart renderer with dense cap glyphs, today highlight, and cursor scrubber.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use rotameter_client::types::TimeseriesRow;

use crate::fmt::{dimension_key, format_count, short_bucket_label};
use crate::state::AppState;

const GLYPHS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Renders the ASCII block time-series chart, range baseline, cursor readout, and active filter line.
pub fn render_chart(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 4 || inner.width < 10 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(3)])
        .split(inner);

    let ts: &[TimeseriesRow] = app.query.as_ref().map_or(&[], |q| q.timeseries.as_slice());
    let bars_lines = build_bar_lines(
        ts,
        app.scrub_idx,
        usize::from(chunks[0].width),
        usize::from(chunks[0].height),
    );
    frame.render_widget(Paragraph::new(bars_lines), chunks[0]);
    frame.render_widget(
        Paragraph::new(build_footer_lines(app, inner.width)),
        chunks[1],
    );
}

fn build_bar_lines(
    ts: &[TimeseriesRow],
    scrub_idx: Option<usize>,
    width: usize,
    height: usize,
) -> Vec<Line<'static>> {
    if ts.is_empty() || height == 0 || width == 0 {
        return vec![Line::from(Span::styled(
            "no time-series data",
            Style::default().fg(Color::DarkGray),
        ))];
    }

    let max_bars = width.div_ceil(2);
    let start_idx = ts.len().saturating_sub(max_bars);
    let slice = &ts[start_idx..];
    let peak = slice.iter().map(|b| b.visitors.max(1)).max().unwrap_or(1);
    let total_levels = height * 8;
    let mut rows = Vec::with_capacity(height);

    for row_idx in 0..height {
        let level_floor = (height - 1 - row_idx) * 8;
        let mut spans = Vec::with_capacity(slice.len() * 2);
        for (offset, b) in slice.iter().enumerate() {
            if offset > 0 {
                spans.push(Span::raw(" "));
            }
            let global_idx = start_idx + offset;
            let fill =
                usize::try_from((b.visitors * u64::try_from(total_levels).unwrap_or(1)) / peak)
                    .unwrap_or(total_levels);
            let ch = cell_glyph(fill, level_floor, row_idx == 0 && b.visitors == peak);
            let style = bar_style(global_idx, ts.len() - 1, scrub_idx);
            spans.push(Span::styled(ch.to_string(), style));
        }
        rows.push(Line::from(spans));
    }
    rows
}

fn cell_glyph(fill: usize, level_floor: usize, is_peak_top: bool) -> char {
    if fill <= level_floor {
        return ' ';
    }
    let rem = fill - level_floor;
    if rem >= 8 {
        '█'
    } else if is_peak_top && rem == 7 {
        '▀'
    } else {
        GLYPHS[rem]
    }
}

fn bar_style(idx: usize, last_idx: usize, scrub_idx: Option<usize>) -> Style {
    if scrub_idx == Some(idx) {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else if idx == last_idx {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    }
}

fn build_footer_lines(app: &AppState, width: u16) -> Vec<Line<'static>> {
    let axis = build_axis_line(app, usize::from(width));
    let cursor = if let Some(b) = app.active_bucket() {
        let lbl = short_bucket_label(&b.bucket, app.range);
        let tag = if app.scrub_idx.is_some() {
            "CURSOR"
        } else {
            "LATEST"
        };
        Line::from(Span::styled(
            format!(
                "{tag}: {lbl} · {} VISITORS · {} VIEWS",
                format_count(b.visitors),
                format_count(b.pageviews)
            ),
            Style::default().fg(Color::Yellow),
        ))
    } else {
        Line::from(Span::styled(
            "CURSOR: --",
            Style::default().fg(Color::DarkGray),
        ))
    };

    let filter_line = if app.filters.is_empty() {
        Line::from(Span::styled(
            "ACTIVE FILTER: none",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        let tags: Vec<String> = app
            .filters
            .iter()
            .map(|f| {
                let k = dimension_key(f.dimension);
                let v = match &f.value {
                    rotameter_client::types::FilterValue::Scalar(s) => s.clone(),
                    rotameter_client::types::FilterValue::List(xs) => xs.join(","),
                };
                format!("[{k}={v}]")
            })
            .collect();
        Line::from(Span::styled(
            format!("ACTIVE FILTER: {} (Esc to clear)", tags.join(" ")),
            Style::default().fg(Color::Magenta),
        ))
    };

    vec![axis, cursor, filter_line]
}

fn build_axis_line(app: &AppState, width: usize) -> Line<'static> {
    let ts: &[TimeseriesRow] = app.query.as_ref().map_or(&[], |q| q.timeseries.as_slice());
    let (Some(first), Some(last)) = (ts.first(), ts.last()) else {
        return Line::from("");
    };
    let left = short_bucket_label(&first.bucket, app.range);
    let end_short = last
        .bucket
        .get(5..10)
        .unwrap_or(&last.bucket)
        .replace('-', ".");
    let right = format!("TODAY ({end_short})");
    let fill = width.saturating_sub(left.len() + right.len() + 3).max(2);
    Line::from(Span::styled(
        format!("{left} {}▶ {right}", "─".repeat(fill)),
        Style::default().fg(Color::DarkGray),
    ))
}
