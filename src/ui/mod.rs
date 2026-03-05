pub mod gmail_ui;
pub mod calendar_ui;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap};
use ratatui::Frame;

use crate::app::{ActivePane, ActiveTab, App, UiMode};
use crate::modal::{EditorField, EditorMode, ModalState};
use crate::workspace::{CalendarCompactView, ModuleView, TasksCompactView, GmailCompactView};

pub fn draw(frame: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_tabs(frame, app, root[0]);
    draw_header(frame, app, root[1]);

    match &app.focus.mode {
        UiMode::Focused(_) => match app.active_tab {
            ActiveTab::Tasks => draw_tasks(frame, app, root[2]),
            ActiveTab::Gmail => gmail_ui::draw_gmail(frame, &app.gmail, root[2]),
            ActiveTab::Calendar => calendar_ui::draw_calendar(frame, &app.calendar, root[2]),
        },
        UiMode::CommandCenter => draw_dashboard(frame, app, root[2]),
    }

    draw_footer(frame, app, root[3]);
    draw_status(frame, app, root[4]);

    if app.show_help {
        draw_help_overlay(frame, frame.area());
    }
    if app.command_palette_open {
        draw_command_palette(frame, app, frame.area());
    }

    if let Some(modal) = &app.modal {
        draw_modal(frame, modal, frame.area());
    }
}

fn draw_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let titles = ["Tasks", "Gmail", "Calendar"].into_iter().map(Line::from).collect::<Vec<_>>();
    let selected = match app.active_tab {
        ActiveTab::Tasks => 0,
        ActiveTab::Gmail => 1,
        ActiveTab::Calendar => 2,
    };

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("gtui"))
        .select(selected)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    frame.render_widget(tabs, area);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let title = match app.active_tab {
        ActiveTab::Tasks => format!(" account: {} | list: {} ", app.account_label, app.selected_tasklist_title()),
        ActiveTab::Gmail => format!(" account: {} | label: {} ", app.account_label, app.gmail.selected_label_name()),
        ActiveTab::Calendar => format!(
            " account: {} | calendar: {} | range: {} -> {} ",
            app.account_label,
            app.calendar
                .calendars
                .get(app.calendar.selected_calendar)
                .map(|c| c.summary.clone())
                .unwrap_or_else(|| "None".to_string()),
            app.calendar.range_start.format("%Y-%m-%d"),
            app.calendar.range_end().format("%Y-%m-%d")
        ),
    };
    let mode = match app.focus.mode {
        UiMode::Focused(_) => "Focused",
        UiMode::CommandCenter => "Command Center",
    };
    frame.render_widget(Paragraph::new(format!("{title} | mode: {mode}")), area);
}

fn draw_dashboard(frame: &mut Frame, app: &App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[1]);

    let mut tasks = TasksCompactView { tasks: app.tasks.clone(), selected: app.selected_task_view };
    tasks.draw(frame, layout[0], app.focus.focused_tile == crate::app::DashboardTile::Tasks);
    let mut cal = CalendarCompactView { state: app.calendar.clone() };
    cal.draw(frame, right[0], app.focus.focused_tile == crate::app::DashboardTile::Calendar);
    let mut gmail = GmailCompactView { state: app.gmail.clone(), detail: app.gmail.detail.clone() };
    gmail.draw(frame, right[1], app.focus.focused_tile == crate::app::DashboardTile::Gmail);
}

fn draw_tasks(frame: &mut Frame, app: &App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(40),
            Constraint::Percentage(35),
        ])
        .split(area);

    let list_border = if app.active_pane == ActivePane::TaskLists { Style::default().fg(Color::Green) } else { Style::default() };
    let task_border = if app.active_pane == ActivePane::Tasks { Style::default().fg(Color::Green) } else { Style::default() };
    let detail_border = if app.active_pane == ActivePane::Details { Style::default().fg(Color::Green) } else { Style::default() };

    let tasklist_items: Vec<ListItem> = app
        .tasklists
        .iter()
        .enumerate()
        .map(|(i, l)| ListItem::new(format!("{} {}", if i == app.selected_tasklist { ">" } else { " " }, l.title)))
        .collect();
    frame.render_widget(
        List::new(tasklist_items).block(Block::default().borders(Borders::ALL).title("Tasklists").border_style(list_border)),
        columns[0],
    );

    let filtered = app.filtered_task_indices();
    let task_items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(view_idx, idx)| {
            let t = &app.tasks[*idx];
            let prefix = if t.is_completed() { "[x]" } else { "[ ]" };
            ListItem::new(format!("{} {prefix} {}", if view_idx == app.selected_task_view { ">" } else { " " }, t.title))
        })
        .collect();
    frame.render_widget(
        List::new(task_items)
            .block(Block::default().borders(Borders::ALL).title(if app.show_completed { "Tasks (all)" } else { "Tasks (incomplete)" }).border_style(task_border)),
        columns[1],
    );

    let details = if let Some(task) = app.selected_task() {
        vec![
            Line::from(vec![Span::styled("Title: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(&task.title)]),
            Line::from(vec![Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(task.status.as_deref().unwrap_or("needsAction"))]),
            Line::from(vec![Span::styled("Due: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(task.due.as_deref().unwrap_or("-"))]),
            Line::from(vec![Span::styled("Updated: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(task.updated.as_deref().unwrap_or("-"))]),
            Line::from(""),
            Line::from(vec![Span::styled("Notes:", Style::default().add_modifier(Modifier::BOLD))]),
            Line::from(task.notes.clone().unwrap_or_else(|| "-".to_string())),
        ]
    } else {
        vec![Line::from("No task selected")]
    };

    frame.render_widget(
        Paragraph::new(details)
            .block(Block::default().borders(Borders::ALL).title("Details").border_style(detail_border))
            .wrap(Wrap { trim: false }),
        columns[2],
    );
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let hints = match app.active_tab {
        ActiveTab::Tasks => "q quit | D command-center | Ctrl+1/2/3 full modules | tab tiles | enter zoom | : commands | ? help",
        ActiveTab::Gmail => "q quit | D command-center | Ctrl+1/2/3 full modules | tab tiles | enter zoom | : commands | ? help",
        ActiveTab::Calendar => "q quit | D command-center | Ctrl+1/2/3 full modules | tab tiles | enter zoom | : commands | ? help",
    };
    frame.render_widget(Paragraph::new(hints), area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let focus_label = format!("{:?}", app.focus.focused_tile);
    let base = if app.loading { format!("{} loading... {}", app.spinner_char(), app.status) } else { app.status.clone() };
    let tile_hint = match app.focus.focused_tile {
        crate::app::DashboardTile::Tasks => "a add | e edit | x complete | d delete | / filter",
        crate::app::DashboardTile::Calendar => "n new | e edit | d delete | t today",
        crate::app::DashboardTile::Gmail => "a archive | u unread | r reply | c compose",
    };
    let status = format!("Focus: {focus_label} | j/k move | tab next | {tile_hint} | {base}");
    frame.render_widget(Paragraph::new(status), area);
}

fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(80, 60, area);
    frame.render_widget(Clear, popup);
    let lines = vec![
        Line::from("Global: q quit, T switch tab, tab switch pane, g refresh, ? toggle help"),
        Line::from("Gmail: enter open, a archive, u unread toggle, r reply, c compose, / search"),
        Line::from("Tasks: a add, e edit, x complete, d delete, / search, c show completed"),
        Line::from("Calendar: n new, e edit, d delete, t today, [/] date range, / search"),
        Line::from("Command Center: tab/backtab focus tile, enter full module view, esc back"),
    ];
    frame.render_widget(Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Help")), popup);
}

fn draw_command_palette(frame: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(70, 20, area);
    frame.render_widget(Clear, popup);
    let body = vec![
        Line::from(format!(":{}", app.command_palette)),
        Line::from("Commands: help, refresh, goto tasks|calendar|gmail, search <q>"),
    ];
    frame.render_widget(
        Paragraph::new(body).block(Block::default().title("Command Palette").borders(Borders::ALL)),
        popup,
    );
}

fn draw_modal(frame: &mut Frame, modal: &ModalState, area: Rect) {
    let popup = centered_rect(70, 50, area);
    frame.render_widget(Clear, popup);

    match modal {
        ModalState::TaskEditor { mode, title, notes, field, quick } => {
            let mode_title = match mode {
                EditorMode::Add => "Add Task",
                EditorMode::Edit => "Edit Task",
            };
            if *quick {
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::from(format!("> title: {title}")),
                        Line::from(""),
                        Line::from("enter submit | esc cancel"),
                    ])
                    .block(Block::default().borders(Borders::ALL).title(format!("{mode_title} (Quick)"))),
                    popup,
                );
            } else {
                let title_label = if *field == EditorField::Title { "> title" } else { "  title" };
                let notes_label = if *field == EditorField::Notes { "> notes" } else { "  notes" };
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::from(format!("{title_label}: {title}")),
                        Line::from(format!("{notes_label}: {notes}")),
                        Line::from(""),
                        Line::from("tab switch field | enter submit | esc cancel"),
                    ])
                    .block(Block::default().borders(Borders::ALL).title(mode_title)),
                    popup,
                );
            }
        }
        ModalState::Search { query } => {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(format!("query: {query}")),
                    Line::from(""),
                    Line::from("enter apply | esc cancel"),
                ])
                .block(Block::default().borders(Borders::ALL).title("Search")),
                popup,
            );
        }
        ModalState::ConfirmDelete => {
            frame.render_widget(
                Paragraph::new(vec![Line::from("Delete selected task?"), Line::from(""), Line::from("y/enter confirm | esc cancel")])
                    .block(Block::default().borders(Borders::ALL).title("Confirm Delete")),
                popup,
            );
        }
        ModalState::Compose { to, subject, body, field, is_reply } => {
            let to_lbl = if *field == EditorField::To { "> to" } else { "  to" };
            let subj_lbl = if *field == EditorField::Subject { "> subject" } else { "  subject" };
            let body_lbl = if *field == EditorField::Body { "> body" } else { "  body" };
            let title = if *is_reply { "Reply" } else { "Compose" };
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(format!("{to_lbl}: {to}")),
                    Line::from(format!("{subj_lbl}: {subject}")),
                    Line::from(format!("{body_lbl}: {body}")),
                    Line::from(""),
                    Line::from("tab switch field | enter submit | esc cancel"),
                ])
                .block(Block::default().borders(Borders::ALL).title(title))
                .wrap(Wrap { trim: false }),
                popup,
            );
        }
        ModalState::CalendarEditor {
            title,
            start,
            end,
            all_day,
            location,
            description,
            field,
            is_edit,
        } => {
            let ttl_lbl = if *field == EditorField::Title { "> title" } else { "  title" };
            let start_lbl = if *field == EditorField::To { "> start" } else { "  start" };
            let end_lbl = if *field == EditorField::Subject { "> end" } else { "  end" };
            let loc_lbl = if *field == EditorField::Body { "> location" } else { "  location" };
            let desc_lbl = if *field == EditorField::Notes { "> notes" } else { "  notes" };
            let title_text = if *is_edit { "Edit Event" } else { "New Event" };
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(format!("{ttl_lbl}: {title} (all-day toggle: space) [{}]", if *all_day { "on" } else { "off" })),
                    Line::from(format!("{start_lbl}: {start}")),
                    Line::from(format!("{end_lbl}: {end}")),
                    Line::from(format!("{loc_lbl}: {location}")),
                    Line::from(format!("{desc_lbl}: {description}")),
                    Line::from(""),
                    Line::from("tab next field | enter save | esc cancel"),
                ])
                .block(Block::default().borders(Borders::ALL).title(title_text))
                .wrap(Wrap { trim: false }),
                popup,
            );
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
