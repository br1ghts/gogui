use crossterm::event::KeyEvent;
use chrono::Local;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::{layout::Rect, Frame};

use crate::app::calendar_state::CalendarState;
use crate::app::gmail_state::GmailState;
use crate::calendar::models::EventTime;
use crate::gmail::models::MessageDetail;
use crate::models::Task;

#[allow(dead_code)]
pub trait ModuleView {
    fn title(&self) -> &str;
    fn draw(&mut self, f: &mut Frame, area: Rect, focused: bool);
    fn handle_key(&mut self, key: KeyEvent) -> bool;
    fn refresh(&mut self);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleKind {
    Tasks,
    Gmail,
    Calendar,
}

pub struct TasksCompactView {
    pub tasks: Vec<Task>,
    pub selected: usize,
}

impl ModuleView for TasksCompactView {
    fn title(&self) -> &str {
        "Tasks"
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        let border = if focused {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };
        let now = chrono::Utc::now();
        let today = now.date_naive();
        let mut overdue = 0usize;
        let mut due_today = 0usize;
        let mut incomplete = 0usize;
        for t in &self.tasks {
            if !t.is_completed() {
                incomplete += 1;
                if let Some(due) = &t.due {
                    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(due) {
                        if parsed.date_naive() < today {
                            overdue += 1;
                        } else if parsed.date_naive() == today {
                            due_today += 1;
                        }
                    }
                }
            }
        }
        let title = if focused {
            format!("▶ Tasks ({} overdue | {} today | {} open)", overdue, due_today, incomplete)
        } else {
            format!("Tasks ({} overdue | {} today | {} open)", overdue, due_today, incomplete)
        };
        let items: Vec<ListItem> = self
            .tasks
            .iter()
            .enumerate()
            .take(12)
            .map(|(i, t)| ListItem::new(format!("{} {}", if i == self.selected { ">" } else { " " }, t.title)))
            .collect();
        f.render_widget(
            List::new(items).block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(border)
                    .border_type(if focused { BorderType::Double } else { BorderType::Plain }),
            ),
            area,
        );
    }

    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        false
    }

    fn refresh(&mut self) {}
}

pub struct GmailCompactView {
    pub state: GmailState,
    pub detail: Option<MessageDetail>,
}

impl ModuleView for GmailCompactView {
    fn title(&self) -> &str {
        "Gmail"
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        let border = if focused {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };
        let unread_count = self.state.threads.iter().filter(|t| t.unread).count();
        let title = if focused {
            format!("▶ Gmail: INBOX ({} unread)", unread_count)
        } else {
            format!("Gmail: INBOX ({} unread)", unread_count)
        };
        let outer = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border)
            .border_type(if focused { BorderType::Double } else { BorderType::Plain });
        let inner = outer.inner(area);
        f.render_widget(outer, area);

        if inner.height < 6 || inner.width < 24 {
            f.render_widget(Paragraph::new("Gmail tile too small"), inner);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);

        let items: Vec<ListItem> = self
            .state
            .threads
            .iter()
            .enumerate()
            .take(8)
            .map(|(i, t)| {
                let from_short = t.from.chars().take(14).collect::<String>();
                let subject = t.subject.chars().take(28).collect::<String>();
                ListItem::new(format!(
                    "{} {} {} | {}",
                    if i == self.state.selected_thread { ">" } else { " " },
                    if t.unread { "●" } else { " " },
                    from_short,
                    subject
                ))
            })
            .collect();
        f.render_widget(List::new(items).block(Block::default().title("Threads").borders(Borders::BOTTOM)), chunks[0]);

        let preview_lines = if let Some(d) = &self.detail {
            vec![
                Line::from(format!("From: {}", d.from)),
                Line::from(format!("Subject: {}", d.subject)),
                Line::from(""),
                Line::from(d.body.chars().take(600).collect::<String>()),
            ]
        } else {
            vec![Line::from("Select a thread to read")]
        };
        f.render_widget(
            Paragraph::new(preview_lines)
                .block(Block::default().title("Preview"))
                .wrap(Wrap { trim: true }),
            chunks[1],
        );
    }

    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        false
    }

    fn refresh(&mut self) {}
}

pub struct CalendarCompactView {
    pub state: CalendarState,
}

impl ModuleView for CalendarCompactView {
    fn title(&self) -> &str {
        "Calendar"
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        let border = if focused {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };
        let title = if focused {
            "▶ Calendar: Today + 7 days".to_string()
        } else {
            "Calendar: Today + 7 days".to_string()
        };
        let now_local = Local::now();
        let today = now_local.date_naive();
        let mut lines: Vec<Line> = Vec::new();
        let mut last_group = String::new();
        for (i, (_, e)) in self.state.filtered_events().iter().take(12).enumerate() {
            let (day, time_label, ongoing) = match &e.start {
                EventTime::AllDay(d) => {
                    let grp = if *d == today {
                        "Today".to_string()
                    } else if *d == today.succ_opt().unwrap_or(today) {
                        "Tomorrow".to_string()
                    } else {
                        d.format("%a %b %-d").to_string()
                    };
                    (grp, "All-day".to_string(), false)
                }
                EventTime::DateTime(dt) => {
                    let d = dt.date_naive();
                    let grp = if d == today {
                        "Today".to_string()
                    } else if d == today.succ_opt().unwrap_or(today) {
                        "Tomorrow".to_string()
                    } else {
                        d.format("%a %b %-d").to_string()
                    };
                    let ts = dt.with_timezone(&Local).format("%H:%M").to_string();
                    let ongoing = now_local >= dt.with_timezone(&Local)
                        && match &e.end {
                            EventTime::DateTime(end) => now_local <= end.with_timezone(&Local),
                            EventTime::AllDay(_) => false,
                        };
                    (grp, ts, ongoing)
                }
            };
            if day != last_group {
                lines.push(Line::from(format!("-- {day} --")));
                last_group = day;
            }
            let loc = if e.location.is_some() { "loc" } else { "-" };
            let now_mark = if ongoing { "NOW" } else { "   " };
            lines.push(Line::from(format!(
                "{} {} {} {} {}",
                if i == self.state.selected_event { ">" } else { " " },
                now_mark,
                time_label,
                loc,
                e.title
            )));
        }
        f.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(border)
                        .border_type(if focused { BorderType::Double } else { BorderType::Plain }),
                )
                .wrap(Wrap { trim: true }),
            area,
        );
    }

    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        false
    }

    fn refresh(&mut self) {}
}
