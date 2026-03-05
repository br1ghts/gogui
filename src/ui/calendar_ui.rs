use chrono::{Datelike, Local};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::calendar_state::{CalendarPane, CalendarState};
use crate::calendar::models::EventTime;

pub fn draw_calendar(frame: &mut Frame, state: &CalendarState, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(24),
            Constraint::Percentage(41),
            Constraint::Percentage(35),
        ])
        .split(area);

    draw_calendars(frame, state, cols[0]);
    draw_agenda(frame, state, cols[1]);
    draw_details(frame, state, cols[2]);
}

fn draw_calendars(frame: &mut Frame, state: &CalendarState, area: Rect) {
    let border = if state.active_pane == CalendarPane::Calendars { Style::default().fg(Color::Green) } else { Style::default() };
    let items: Vec<ListItem> = state
        .calendars
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let p = if i == state.selected_calendar { ">" } else { " " };
            let primary = if c.primary { "*" } else { " " };
            ListItem::new(format!("{p}{primary} {}", c.summary))
        })
        .collect();

    frame.render_widget(List::new(items).block(Block::default().borders(Borders::ALL).title("Calendars").border_style(border)), area);
}

fn draw_agenda(frame: &mut Frame, state: &CalendarState, area: Rect) {
    let border = if state.active_pane == CalendarPane::Agenda { Style::default().fg(Color::Green) } else { Style::default() };
    let mut items: Vec<ListItem> = Vec::new();
    let now_date = Local::now().date_naive();
    let mut last_group = String::new();

    for (view_i, (_, e)) in state.filtered_events().iter().enumerate() {
        let header = match &e.start {
            EventTime::AllDay(d) => {
                if *d == now_date {
                    "Today".to_string()
                } else if *d == now_date.succ_opt().unwrap_or(now_date) {
                    "Tomorrow".to_string()
                } else if d.iso_week() == now_date.iso_week() {
                    "This Week".to_string()
                } else {
                    "Later".to_string()
                }
            }
            EventTime::DateTime(dt) => {
                let d = dt.date_naive();
                if d == now_date {
                    "Today".to_string()
                } else if d == now_date.succ_opt().unwrap_or(now_date) {
                    "Tomorrow".to_string()
                } else if d.iso_week() == now_date.iso_week() {
                    "This Week".to_string()
                } else {
                    "Later".to_string()
                }
            }
        };

        if last_group != header {
            items.push(ListItem::new(format!("-- {header} --")));
            last_group = header.clone();
        }

        let start_txt = match &e.start {
            EventTime::AllDay(d) => format!("{} all-day", d),
            EventTime::DateTime(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        };
        let loc = if e.location.is_some() { "loc" } else { "-" };
        let busy = if e.is_free() { "free" } else { "busy" };
        let sel = if view_i == state.selected_event { ">" } else { " " };
        items.push(ListItem::new(format!("{sel} {start_txt} | {} {loc} [{busy}]", e.title)));
    }

    frame.render_widget(List::new(items).block(Block::default().borders(Borders::ALL).title("Agenda").border_style(border)), area);
}

fn draw_details(frame: &mut Frame, state: &CalendarState, area: Rect) {
    let border = if state.active_pane == CalendarPane::Detail { Style::default().fg(Color::Green) } else { Style::default() };
    let lines = if let Some(e) = state.selected_event() {
        let when = match (&e.start, &e.end) {
            (EventTime::AllDay(s), EventTime::AllDay(en)) => format!("{} to {} (all-day)", s, en),
            (EventTime::DateTime(s), EventTime::DateTime(en)) => format!("{} to {}", s, en),
            _ => "mixed event time".to_string(),
        };
        vec![
            Line::from(vec![Span::styled("Title: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(e.title.clone())]),
            Line::from(vec![Span::styled("When: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(when)]),
            Line::from(vec![Span::styled("Timezone: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(e.timezone.clone().unwrap_or_default())]),
            Line::from(vec![Span::styled("Location: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(e.location.clone().unwrap_or_default())]),
            Line::from(vec![Span::styled("Attendees: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(e.attendees_count.to_string())]),
            Line::from(vec![Span::styled("Meet: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(e.meet_link.clone().unwrap_or_default())]),
            Line::from(""),
            Line::from(e.description.clone().unwrap_or_default()),
        ]
    } else {
        vec![Line::from("No event selected")]
    };

    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Event Details").border_style(border)).wrap(Wrap { trim: false }),
        area,
    );
}
