#![forbid(unsafe_code)]
//! Centered modal overlays for site switching and keybinding help.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::state::AppState;

fn centered_rect(w: u16, h: u16, area: Rect) -> Rect {
    let cw = w.min(area.width);
    let ch = h.min(area.height);
    Rect::new(
        area.x + (area.width - cw) / 2,
        area.y + (area.height - ch) / 2,
        cw,
        ch,
    )
}

pub(crate) fn render_site_switcher(frame: &mut Frame<'_>, app: &AppState) {
    let area = centered_rect(48, 12, frame.area());
    frame.render_widget(Clear, area);
    let mut lines: Vec<Line<'static>> = if app.sites.is_empty() {
        vec![Line::from("No sites listed (press Esc to close)")]
    } else {
        app.sites
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let mark = if i == app.site_picker_idx { ">" } else { " " };
                let style = if i == app.site_picker_idx {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Line::from(Span::styled(format!(" {mark} {s}"), style))
            })
            .collect()
    };
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " [Enter] select  [Esc] cancel",
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" SWITCH SITE "),
        ),
        area,
    );
}

pub(crate) fn render_help_modal(frame: &mut Frame<'_>) {
    let area = centered_rect(56, 13, frame.area());
    frame.render_widget(Clear, area);
    let lines = vec![
        Line::from(" 1-5       Switch time range (24H / 7D / 30D / 12M / ALL)"),
        Line::from(" ← / → / h / l   Scrub chart bar & update cursor readout"),
        Line::from(" ↑ / ↓ / j / k   Select row in active breakdown table"),
        Line::from(" Enter     Push selected row as drill-down filter"),
        Line::from(" Esc       Pop active drill-down filter / clear cursor"),
        Line::from(" Tab       Cycle panel (PAGES / SOURCES / GEO / TECH / EVENTS)"),
        Line::from(" s         Open site switcher"),
        Line::from(" r         Force immediate refresh"),
        Line::from(" ? / q     Toggle help / Quit"),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" KEYBINDINGS "),
        ),
        area,
    );
}
