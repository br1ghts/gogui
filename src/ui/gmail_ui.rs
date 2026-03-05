use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::gmail_state::{GmailPane, GmailState};

pub fn draw_gmail(frame: &mut Frame, state: &GmailState, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(22),
            Constraint::Percentage(43),
            Constraint::Percentage(35),
        ])
        .split(area);

    draw_labels(frame, state, columns[0]);
    draw_threads(frame, state, columns[1]);
    draw_detail(frame, state, columns[2]);
}

fn draw_labels(frame: &mut Frame, state: &GmailState, area: Rect) {
    let style = if state.active_pane == GmailPane::Labels {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };

    let items: Vec<ListItem> = state
        .labels
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let prefix = if i == state.selected_label { ">" } else { " " };
            ListItem::new(format!("{prefix} {}", l.name))
        })
        .collect();

    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title("Labels").border_style(style)),
        area,
    );
}

fn draw_threads(frame: &mut Frame, state: &GmailState, area: Rect) {
    let style = if state.active_pane == GmailPane::Threads {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };

    let items: Vec<ListItem> = state
        .threads
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let unread = if t.unread { "●" } else { " " };
            let prefix = if i == state.selected_thread { ">" } else { " " };
            ListItem::new(format!(
                "{prefix}{unread} {} | {} | {}\n   {}",
                t.from, t.subject, t.date, t.snippet
            ))
        })
        .collect();

    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title("Threads").border_style(style)),
        area,
    );
}

fn draw_detail(frame: &mut Frame, state: &GmailState, area: Rect) {
    let style = if state.active_pane == GmailPane::Detail {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };

    let lines = if let Some(m) = &state.detail {
        vec![
            Line::from(vec![Span::styled("From: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(&m.from)]),
            Line::from(vec![Span::styled("To: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(&m.to)]),
            Line::from(vec![Span::styled("Subject: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(&m.subject)]),
            Line::from(vec![Span::styled("Date: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(&m.date)]),
            Line::from(vec![Span::styled("Labels: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(m.labels.join(", "))]),
            Line::from(""),
            Line::from(m.body.clone()),
        ]
    } else {
        vec![Line::from("No thread selected")]
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Message").border_style(style))
            .wrap(Wrap { trim: false }),
        area,
    );
}
