mod app;
mod api_error;
mod auth;
mod calendar;
mod gmail;
mod google;
mod input;
mod modal;
mod models;
mod storage;
mod ui;
mod workspace;
mod runtime;

use std::io;
use std::time::Duration;

use app::{ActiveTab, App};
use api_error::{actionable_message, ApiError};
use calendar::actions::build_event_request;
use calendar::client::CalendarClient;
use calendar::models::EventEdit;
use crossterm::event::{Event as CEvent, EventStream};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, terminal};
use futures::StreamExt;
use gmail::actions::build_raw_email;
use gmail::client::GmailClient;
use google::GoogleTasksClient;
use input::AppAction;
use modal::{ModalState, ModalSubmit};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::time::interval;
use runtime::{route_key, routed_module, RefreshGate, RoutedModule};
use app::{DashboardTile, UiMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let run_result = run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    terminal.show_cursor()?;

    if let Err(err) = run_result {
        eprintln!("gtui error: {err}");
    }

    Ok(())
}

async fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), String> {
    let mut app = App::new("Google".to_string());
    let mut tasks_auth = auth::AuthManager::from_disk_or_authorize().await?;
    let tasks_client = GoogleTasksClient::new();
    let mut gmail_client = GmailClient::from_disk_or_authorize().await.map_err(|e| e.to_string())?;
    let mut calendar_client = CalendarClient::from_disk_or_authorize()
        .await
        .map_err(|e| e.to_string())?;

    refresh_tasks_all(&mut app, &mut tasks_auth, &tasks_client).await;
    refresh_gmail_all(&mut app, &mut gmail_client).await;
    refresh_calendar_all(&mut app, &mut calendar_client).await;

    let mut reader = EventStream::new();
    let mut ticker = interval(Duration::from_millis(33));
    let mut refresh_gate = RefreshGate::new(Duration::from_millis(400));
    let mut dirty = true;

    while !app.should_quit {
        if dirty {
            terminal
                .draw(|f| ui::draw(f, &app))
                .map_err(|e| format!("Terminal draw failed: {e}"))?;
            dirty = false;
        }

        tokio::select! {
            _ = ticker.tick() => {
                app.tick();
                dirty = true;
            },
            maybe_event = reader.next() => {
                if let Some(Ok(CEvent::Key(key))) = maybe_event {
                    dirty = true;
                    if app.command_palette_open {
                        match key.code {
                            crossterm::event::KeyCode::Esc => {
                                app.command_palette_open = false;
                            }
                            crossterm::event::KeyCode::Backspace => {
                                app.command_palette.pop();
                            }
                            crossterm::event::KeyCode::Enter => {
                                run_command_palette(&mut app).await;
                                app.command_palette_open = false;
                            }
                            crossterm::event::KeyCode::Char(c) => {
                                app.command_palette.push(c);
                            }
                            _ => {}
                        }
                        continue;
                    }
                    let outcome = route_key(&mut app, key);
                    if let Some(submit) = outcome.modal_submit {
                        app.close_modal();
                        handle_modal_submit(
                            &mut app,
                            &mut tasks_auth,
                            &tasks_client,
                            &mut gmail_client,
                            &mut calendar_client,
                            submit,
                        )
                        .await;
                        continue;
                    }

                    for action in outcome.actions {
                        handle_action(
                            &mut app,
                            &mut tasks_auth,
                            &tasks_client,
                            &mut gmail_client,
                            &mut calendar_client,
                            &mut refresh_gate,
                            action,
                        )
                        .await;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_action(
    app: &mut App,
    auth: &mut auth::AuthManager,
    tasks_client: &GoogleTasksClient,
    gmail_client: &mut GmailClient,
    calendar_client: &mut CalendarClient,
    refresh_gate: &mut RefreshGate,
    action: AppAction,
) {
    let target = routed_module(app);
    match action {
        AppAction::None => {}
        AppAction::Quit => app.should_quit = true,
        AppAction::SwitchTab => app.switch_tab(),
        AppAction::LoadTabTasks => app.set_tab(ActiveTab::Tasks),
        AppAction::LoadTabGmail => app.set_tab(ActiveTab::Gmail),
        AppAction::LoadTabCalendar => app.set_tab(ActiveTab::Calendar),
        AppAction::ToggleDashboard => app.toggle_dashboard(),
        AppAction::OpenCommandPalette => app.command_palette_open = true,
        AppAction::RefreshVisible => {
            if refresh_gate.try_start("all") {
                refresh_tasks_all(app, auth, tasks_client).await;
                refresh_gmail_all(app, gmail_client).await;
                refresh_calendar_all(app, calendar_client).await;
                refresh_gate.finish("all");
            }
        }
        AppAction::NextTile => {
            if app.focus.mode == UiMode::CommandCenter {
                app.cycle_tile(false);
            } else if matches!(target, RoutedModule::Gmail) {
                app.gmail.pane_next();
            } else if matches!(target, RoutedModule::Calendar) {
                app.calendar.pane_next();
            } else {
                app.pane_right();
            }
        }
        AppAction::PrevTile => {
            if app.focus.mode == UiMode::CommandCenter {
                app.cycle_tile(true);
            } else if matches!(target, RoutedModule::Calendar) {
                app.calendar.pane_prev();
            } else {
                app.pane_right();
            }
        }
        AppAction::ZoomTile => {
            if app.focus.mode == UiMode::CommandCenter {
                app.focus.mode = match app.focus.focused_tile {
                    DashboardTile::Tasks => UiMode::Focused(crate::workspace::ModuleKind::Tasks),
                    DashboardTile::Calendar => UiMode::Focused(crate::workspace::ModuleKind::Calendar),
                    DashboardTile::Gmail => UiMode::Focused(crate::workspace::ModuleKind::Gmail),
                };
            } else if matches!(target, RoutedModule::Gmail) {
                open_gmail_detail(app, gmail_client).await;
            } else {
                app.focus_details();
            }
        }
        AppAction::ExitZoom => {
            app.focus.mode = UiMode::CommandCenter;
        }
        AppAction::ToggleHelp => app.toggle_help(),
        AppAction::MoveUp => {
            if app.focus.mode == UiMode::CommandCenter && matches!(target, RoutedModule::Tasks) {
                if app.selected_task_view > 0 {
                    app.selected_task_view -= 1;
                }
            } else if app.focus.mode == UiMode::CommandCenter && matches!(target, RoutedModule::Gmail) {
                if app.gmail.selected_thread > 0 {
                    app.gmail.selected_thread -= 1;
                    open_gmail_detail(app, gmail_client).await;
                }
            } else if app.focus.mode == UiMode::CommandCenter && matches!(target, RoutedModule::Calendar) {
                if app.calendar.selected_event > 0 {
                    app.calendar.selected_event -= 1;
                }
            } else if matches!(target, RoutedModule::Gmail) {
                let before = app.gmail.selected_label;
                app.gmail.move_up();
                if app.gmail.selected_label != before {
                    refresh_gmail_threads(app, gmail_client).await;
                }
            } else if matches!(target, RoutedModule::Calendar) {
                let before = app.calendar.selected_calendar;
                app.calendar.move_up();
                if before != app.calendar.selected_calendar {
                    refresh_calendar_events(app, calendar_client).await;
                }
            } else {
                let before = app.selected_tasklist;
                app.move_up();
                if before != app.selected_tasklist {
                    refresh_tasks(app, auth, tasks_client).await;
                }
            }
        }
        AppAction::MoveDown => {
            if app.focus.mode == UiMode::CommandCenter && matches!(target, RoutedModule::Tasks) {
                let len = app.filtered_task_indices().len();
                if app.selected_task_view + 1 < len {
                    app.selected_task_view += 1;
                }
            } else if app.focus.mode == UiMode::CommandCenter && matches!(target, RoutedModule::Gmail) {
                if app.gmail.selected_thread + 1 < app.gmail.threads.len() {
                    app.gmail.selected_thread += 1;
                    open_gmail_detail(app, gmail_client).await;
                }
            } else if app.focus.mode == UiMode::CommandCenter && matches!(target, RoutedModule::Calendar) {
                let len = app.calendar.filtered_events().len();
                if app.calendar.selected_event + 1 < len {
                    app.calendar.selected_event += 1;
                }
            } else if matches!(target, RoutedModule::Gmail) {
                let before = app.gmail.selected_label;
                app.gmail.move_down();
                if app.gmail.selected_label != before {
                    refresh_gmail_threads(app, gmail_client).await;
                }
            } else if matches!(target, RoutedModule::Calendar) {
                let before = app.calendar.selected_calendar;
                app.calendar.move_down();
                if before != app.calendar.selected_calendar {
                    refresh_calendar_events(app, calendar_client).await;
                }
            } else {
                let before = app.selected_tasklist;
                app.move_down();
                if before != app.selected_tasklist {
                    refresh_tasks(app, auth, tasks_client).await;
                }
            }
        }
        AppAction::NextPane => {
            if matches!(target, RoutedModule::Gmail) {
                app.gmail.pane_next();
            } else if matches!(target, RoutedModule::Calendar) {
                app.calendar.pane_next();
            } else {
                app.pane_right();
            }
        }
        AppAction::PrevPane => {
            if matches!(target, RoutedModule::Calendar) {
                app.calendar.pane_prev();
            } else {
                app.pane_right();
            }
        }
        AppAction::Refresh => {
            let key = if matches!(target, RoutedModule::Gmail) {
                "gmail"
            } else if matches!(target, RoutedModule::Calendar) {
                "calendar"
            } else {
                "tasks"
            };
            if refresh_gate.try_start(key) {
                if matches!(target, RoutedModule::Gmail) {
                    refresh_gmail_all(app, gmail_client).await;
                } else if matches!(target, RoutedModule::Calendar) {
                    refresh_calendar_all(app, calendar_client).await;
                } else {
                    refresh_tasks_all(app, auth, tasks_client).await;
                }
                refresh_gate.finish(key);
            }
        }
        AppAction::ToggleShowCompleted => {
            if matches!(target, RoutedModule::Gmail) {
                app.modal = Some(ModalState::Compose {
                    to: String::new(),
                    subject: String::new(),
                    body: String::new(),
                    field: modal::EditorField::To,
                    is_reply: false,
                });
            } else {
                app.toggle_completed_filter();
                refresh_tasks(app, auth, tasks_client).await;
            }
        }
        AppAction::OpenAdd => {
            if matches!(target, RoutedModule::Gmail) {
                archive_selected_thread(app, gmail_client).await;
            } else if app.focus.mode == UiMode::CommandCenter && matches!(target, RoutedModule::Tasks) {
                app.open_add_modal_quick();
            } else {
                app.open_add_modal();
            }
        }
        AppAction::OpenEdit => {
            if matches!(target, RoutedModule::Calendar) {
                open_calendar_editor(app, true);
            } else if app.focus.mode == UiMode::CommandCenter && matches!(target, RoutedModule::Tasks) {
                app.open_edit_modal_quick();
            } else {
                app.open_edit_modal();
            }
        }
        AppAction::ToggleComplete => toggle_complete(app, auth, tasks_client).await,
        AppAction::OpenDelete => {
            if matches!(target, RoutedModule::Calendar) {
                app.modal = Some(ModalState::ConfirmDelete);
            } else {
                app.open_delete_modal();
            }
        }
        AppAction::OpenSearch => {
            if matches!(target, RoutedModule::Gmail) {
                app.modal = Some(ModalState::Search {
                    query: app.gmail.search_query.clone(),
                });
            } else if matches!(target, RoutedModule::Calendar) {
                app.modal = Some(ModalState::Search {
                    query: app.calendar.search_query.clone(),
                });
            } else {
                app.open_search_modal();
            }
        }
        AppAction::CancelModal => app.close_modal(),
        AppAction::GmailArchive => archive_selected_thread(app, gmail_client).await,
        AppAction::GmailUnread => toggle_gmail_unread(app, gmail_client).await,
        AppAction::GmailReply => open_reply_modal(app),
        AppAction::GmailCompose => {
            app.modal = Some(ModalState::Compose {
                to: String::new(),
                subject: String::new(),
                body: String::new(),
                field: modal::EditorField::To,
                is_reply: false,
            });
        }
        AppAction::CalendarNew => {
            if matches!(target, RoutedModule::Calendar) {
                open_calendar_editor(app, false);
            }
        }
        AppAction::CalendarToday => {
            if matches!(target, RoutedModule::Calendar) {
                app.calendar.jump_today();
                refresh_calendar_events(app, calendar_client).await;
            } else {
                app.switch_tab();
            }
        }
        AppAction::CalendarRangeBack => {
            if matches!(target, RoutedModule::Calendar) {
                app.calendar.shift_range(-14);
                refresh_calendar_events(app, calendar_client).await;
            }
        }
        AppAction::CalendarRangeForward => {
            if matches!(target, RoutedModule::Calendar) {
                app.calendar.shift_range(14);
                refresh_calendar_events(app, calendar_client).await;
            }
        }
        AppAction::FocusLeft | AppAction::FocusRight | AppAction::FocusUp | AppAction::FocusDown => {
            if app.focus.mode == UiMode::CommandCenter {
                app.focus.focused_tile = match action {
                    AppAction::FocusLeft => DashboardTile::Tasks,
                    AppAction::FocusUp => DashboardTile::Calendar,
                    AppAction::FocusDown => DashboardTile::Gmail,
                    AppAction::FocusRight => {
                        if app.focus.focused_tile == DashboardTile::Tasks {
                            DashboardTile::Calendar
                        } else {
                            app.focus.focused_tile
                        }
                    }
                    _ => app.focus.focused_tile,
                };
            }
        }
        AppAction::AdjustSplitLeft | AppAction::AdjustSplitRight | AppAction::AdjustSplitUp | AppAction::AdjustSplitDown => {}
    }
}

async fn handle_modal_submit(
    app: &mut App,
    auth: &mut auth::AuthManager,
    client: &GoogleTasksClient,
    gmail_client: &mut GmailClient,
    calendar_client: &mut CalendarClient,
    submit: ModalSubmit,
) {
    match submit {
        ModalSubmit::AddTask { title, notes } => {
            if let Some(tasklist_id) = app.selected_tasklist_id().map(str::to_string) {
                app.set_loading(true);
                match auth.access_token().await {
                    Ok(token) => {
                        let notes_opt = if notes.is_empty() { None } else { Some(notes.as_str()) };
                        match client.add_task(&token, &tasklist_id, &title, notes_opt).await {
                            Ok(_) => {
                                app.set_status("Task added");
                                refresh_tasks(app, auth, client).await;
                            }
                            Err(e) => app.set_status(format!("Add failed: {e}")),
                        }
                    }
                    Err(e) => app.set_status(format!("Auth failed: {e}")),
                }
                app.set_loading(false);
            }
        }
        ModalSubmit::EditTask { title, notes } => {
            if let (Some(tasklist_id), Some(task_idx)) = (
                app.selected_tasklist_id().map(str::to_string),
                app.selected_task_index(),
            ) {
                if let Some(task) = app.tasks.get(task_idx).cloned() {
                    app.set_loading(true);
                    match auth.access_token().await {
                        Ok(token) => {
                            let notes_opt = if notes.is_empty() { None } else { Some(notes.as_str()) };
                            match client.edit_task(&token, &tasklist_id, &task.id, &title, notes_opt).await {
                                Ok(_) => {
                                    app.set_status("Task updated");
                                    refresh_tasks(app, auth, client).await;
                                }
                                Err(e) => app.set_status(format!("Edit failed: {e}")),
                            }
                        }
                        Err(e) => app.set_status(format!("Auth failed: {e}")),
                    }
                    app.set_loading(false);
                }
            }
        }
        ModalSubmit::Search(query) => {
            if matches!(routed_module(app), RoutedModule::Gmail) {
                app.gmail.search_query = query;
                refresh_gmail_threads(app, gmail_client).await;
                app.set_status("Gmail search updated");
            } else if matches!(routed_module(app), RoutedModule::Calendar) {
                app.calendar.search_query = query;
                app.set_status("Calendar search updated");
            } else {
                app.apply_search(query);
                app.set_status("Search updated");
            }
        }
        ModalSubmit::DeleteConfirmed => {
            if matches!(routed_module(app), RoutedModule::Calendar) {
                delete_calendar_event(app, calendar_client).await;
            } else {
                delete_selected(app, auth, client).await;
            }
        }
        ModalSubmit::SendEmail { to, subject, body } => {
            app.set_loading(true);
            let raw = build_raw_email(&to, &subject, &body, None);
            match gmail_client.send_raw_message(raw).await {
                Ok(_) => app.set_status("Message sent"),
                Err(e) => app.set_status(format!("Send failed: {e}")),
            }
            app.set_loading(false);
        }
        ModalSubmit::CalendarSave {
            title,
            start,
            end,
            all_day,
            location,
            description,
            is_edit,
        } => {
            save_calendar_event(
                app,
                calendar_client,
                EventEdit {
                    title,
                    start,
                    end,
                    all_day,
                    location,
                    description,
                },
                is_edit,
            )
            .await;
        }
    }
}

async fn refresh_gmail_all(app: &mut App, gmail_client: &mut GmailClient) {
    app.set_loading(true);
    app.set_status("Refreshing Gmail labels...");

    match gmail_client.list_labels().await {
        Ok(labels) => {
            app.gmail.set_labels(
                labels
                    .into_iter()
                    .filter(|l| matches!(l.name.as_str(), "INBOX" | "SENT" | "DRAFT" | "STARRED" | "IMPORTANT") || l.label_type.as_deref() == Some("user"))
                    .collect(),
            );
            refresh_gmail_threads(app, gmail_client).await;
            app.set_status("Gmail refreshed");
        }
        Err(e) => app.set_status(format!("Gmail labels failed: {e}")),
    }
    app.set_loading(false);
}

async fn run_command_palette(app: &mut App) {
    let cmd = app.command_palette.trim().to_string();
    if cmd.is_empty() {
        return;
    }
    if cmd == ":help" {
        app.show_help = true;
        return;
    }
    if cmd == ":refresh" {
        app.set_status("Use g or Ctrl+r to refresh");
        return;
    }
    if let Some(rest) = cmd.strip_prefix(":goto ") {
        match rest.trim() {
            "tasks" => app.set_tab(ActiveTab::Tasks),
            "calendar" => app.set_tab(ActiveTab::Calendar),
            "gmail" => app.set_tab(ActiveTab::Gmail),
            _ => app.set_status("Unknown module"),
        }
        return;
    }
    if let Some(rest) = cmd.strip_prefix(":search ") {
        match routed_module(app) {
            RoutedModule::Tasks => app.apply_search(rest.to_string()),
            RoutedModule::Calendar => app.calendar.search_query = rest.to_string(),
            RoutedModule::Gmail => app.gmail.search_query = rest.to_string(),
        }
        app.set_status("Search updated");
        return;
    }
    app.set_status("Unknown command");
}

async fn refresh_gmail_threads(app: &mut App, gmail_client: &mut GmailClient) {
    let label = app.gmail.selected_label_id().unwrap_or("INBOX").to_string();
    app.set_loading(true);

    match gmail_client
        .list_threads(&label, Some(app.gmail.search_query.as_str()))
        .await
    {
        Ok(threads) => {
            app.gmail.set_threads(threads);
            app.set_status("Gmail threads loaded");
            open_gmail_detail(app, gmail_client).await;
        }
        Err(e) => {
            app.set_status(format!("Gmail threads failed: {e}"));
            if let Ok(cached) = gmail_client.load_cached_threads(&label) {
                app.gmail.set_threads(cached);
            }
        }
    }
    app.set_loading(false);
}

async fn open_gmail_detail(app: &mut App, gmail_client: &mut GmailClient) {
    if let Some(msg_id) = app
        .gmail
        .selected_thread()
        .and_then(|t| t.last_message_id.as_ref())
        .cloned()
    {
        match gmail_client.get_message_detail(&msg_id).await {
            Ok(detail) => app.set_gmail_detail(Some(detail)),
            Err(e) => app.set_status(format!("Message load failed: {e}")),
        }
    } else {
        app.set_gmail_detail(None);
    }
}

async fn archive_selected_thread(app: &mut App, gmail_client: &mut GmailClient) {
    if let Some(thread_id) = app.gmail.selected_thread_id().map(str::to_string) {
        app.set_loading(true);
        match gmail::actions::archive_thread(gmail_client, &thread_id).await {
            Ok(_) => {
                app.set_status("Thread archived");
                refresh_gmail_threads(app, gmail_client).await;
            }
            Err(e) => app.set_status(format!("Archive failed: {e}")),
        }
        app.set_loading(false);
    }
}

async fn toggle_gmail_unread(app: &mut App, gmail_client: &mut GmailClient) {
    if let Some(thread) = app.gmail.selected_thread().cloned() {
        app.set_loading(true);
        let target_unread = !thread.unread;
        match gmail::actions::toggle_unread(gmail_client, &thread.id, target_unread).await {
            Ok(_) => {
                app.set_status("Unread flag updated");
                refresh_gmail_threads(app, gmail_client).await;
            }
            Err(e) => app.set_status(format!("Unread toggle failed: {e}")),
        }
        app.set_loading(false);
    }
}

fn open_reply_modal(app: &mut App) {
    if let Some(detail) = &app.gmail.detail {
        let subject = if detail.subject.starts_with("Re:") {
            detail.subject.clone()
        } else {
            format!("Re: {}", detail.subject)
        };
        app.modal = Some(ModalState::Compose {
            to: detail.from.clone(),
            subject,
            body: "\n\n".to_string(),
            field: modal::EditorField::Body,
            is_reply: true,
        });
    }
}

fn open_calendar_editor(app: &mut App, is_edit: bool) {
    let (title, start, end, location, description, all_day) = if is_edit {
        if let Some(e) = app.calendar.selected_event() {
            let start = match &e.start {
                calendar::models::EventTime::DateTime(dt) => dt.to_rfc3339(),
                calendar::models::EventTime::AllDay(d) => d.to_string(),
            };
            let end = match &e.end {
                calendar::models::EventTime::DateTime(dt) => dt.to_rfc3339(),
                calendar::models::EventTime::AllDay(d) => d.to_string(),
            };
            (
                e.title.clone(),
                start,
                end,
                e.location.clone().unwrap_or_default(),
                e.description.clone().unwrap_or_default(),
                e.is_all_day(),
            )
        } else {
            (String::new(), String::new(), String::new(), String::new(), String::new(), false)
        }
    } else {
        let now = chrono::Utc::now();
        (
            String::new(),
            now.to_rfc3339(),
            (now + chrono::Duration::hours(1)).to_rfc3339(),
            String::new(),
            String::new(),
            false,
        )
    };

    app.modal = Some(ModalState::CalendarEditor {
        title,
        start,
        end,
        all_day,
        location,
        description,
        field: modal::EditorField::Title,
        is_edit,
    });
}

async fn refresh_calendar_all(app: &mut App, client: &mut CalendarClient) {
    app.set_loading(true);
    match client.preflight_read().await {
        Ok(_) => {}
        Err(e) => {
            app.set_status(actionable_message(&e));
            app.set_loading(false);
            return;
        }
    }

    match client.list_calendars().await {
        Ok(cals) => {
            app.calendar.set_calendars(cals);
            refresh_calendar_events(app, client).await;
            app.set_status("Calendar refreshed");
        }
        Err(e) => app.set_status(actionable_message(&e)),
    }
    app.set_loading(false);
}

async fn refresh_calendar_events(app: &mut App, client: &mut CalendarClient) {
    let cal_id = match app.calendar.selected_calendar_id() {
        Some(id) => id.to_string(),
        None => return,
    };
    app.set_loading(true);
    match client
        .list_events(&cal_id, app.calendar.range_start, app.calendar.range_end())
        .await
    {
        Ok(events) => {
            app.calendar.set_events(events);
            app.set_status("Events loaded");
        }
        Err(e) => app.set_status(actionable_message(&e)),
    }
    app.set_loading(false);
}

async fn save_calendar_event(
    app: &mut App,
    client: &mut CalendarClient,
    edit: EventEdit,
    is_edit: bool,
) {
    let req = match build_event_request(&edit) {
        Ok(r) => r,
        Err(e) => {
            app.set_status(actionable_message(&e));
            return;
        }
    };
    let cal_id = match app.calendar.selected_calendar_id() {
        Some(id) => id.to_string(),
        None => return,
    };
    app.set_loading(true);
    let res: Result<(), ApiError> = if is_edit {
        if let Some(event) = app.calendar.selected_event() {
            client.patch_event(&cal_id, &event.id, &req).await
        } else {
            Err(ApiError::Other("No event selected".to_string()))
        }
    } else {
        client.insert_event(&cal_id, &req).await
    };
    match res {
        Ok(_) => {
            app.set_status("Event saved");
            refresh_calendar_events(app, client).await;
        }
        Err(e) => app.set_status(actionable_message(&e)),
    }
    app.set_loading(false);
}

async fn delete_calendar_event(app: &mut App, client: &mut CalendarClient) {
    let cal_id = match app.calendar.selected_calendar_id() {
        Some(id) => id.to_string(),
        None => return,
    };
    let event_id = match app.calendar.selected_event() {
        Some(e) => e.id.clone(),
        None => return,
    };
    app.set_loading(true);
    match client.delete_event(&cal_id, &event_id).await {
        Ok(_) => {
            app.set_status("Event deleted");
            refresh_calendar_events(app, client).await;
        }
        Err(e) => app.set_status(actionable_message(&e)),
    }
    app.set_loading(false);
}

async fn refresh_tasks_all(app: &mut App, auth: &mut auth::AuthManager, client: &GoogleTasksClient) {
    app.set_loading(true);
    app.set_status("Refreshing tasklists...");

    let token = match auth.access_token().await {
        Ok(t) => t,
        Err(e) => {
            app.set_loading(false);
            app.set_status(format!("Auth failed: {e}"));
            return;
        }
    };

    let lists = match client.list_tasklists(&token).await {
        Ok(v) => v,
        Err(e) => {
            app.set_loading(false);
            app.set_status(format!("Failed loading tasklists: {e}"));
            return;
        }
    };

    app.set_tasklists(lists);
    refresh_tasks(app, auth, client).await;
    app.set_loading(false);
    app.set_status("Refresh complete");
}

async fn refresh_tasks(app: &mut App, auth: &mut auth::AuthManager, client: &GoogleTasksClient) {
    let tasklist_id = match app.selected_tasklist_id() {
        Some(id) => id.to_string(),
        None => {
            app.set_tasks(Vec::new());
            return;
        }
    };

    app.set_loading(true);
    app.set_status("Refreshing tasks...");

    let token = match auth.access_token().await {
        Ok(t) => t,
        Err(e) => {
            app.set_loading(false);
            app.set_status(format!("Auth failed: {e}"));
            return;
        }
    };

    match client.list_tasks(&token, &tasklist_id, app.show_completed).await {
        Ok(tasks) => {
            app.set_tasks(tasks);
            app.set_status("Tasks loaded");
        }
        Err(e) => app.set_status(format!("Failed loading tasks: {e}")),
    }

    app.set_loading(false);
}

async fn toggle_complete(app: &mut App, auth: &mut auth::AuthManager, client: &GoogleTasksClient) {
    let tasklist_id = match app.selected_tasklist_id() {
        Some(id) => id.to_string(),
        None => return,
    };

    let task_idx = match app.selected_task_index() {
        Some(idx) => idx,
        None => return,
    };

    let task = match app.tasks.get(task_idx).cloned() {
        Some(t) => t,
        None => return,
    };

    app.set_loading(true);
    match auth.access_token().await {
        Ok(token) => {
            let target_complete = !task.is_completed();
            match client
                .toggle_complete(&token, &tasklist_id, &task.id, target_complete)
                .await
            {
                Ok(_) => {
                    app.set_status("Task completion updated");
                    refresh_tasks(app, auth, client).await;
                }
                Err(e) => app.set_status(format!("Toggle failed: {e}")),
            }
        }
        Err(e) => app.set_status(format!("Auth failed: {e}")),
    }
    app.set_loading(false);
}

async fn delete_selected(app: &mut App, auth: &mut auth::AuthManager, client: &GoogleTasksClient) {
    let tasklist_id = match app.selected_tasklist_id() {
        Some(id) => id.to_string(),
        None => return,
    };

    let task_idx = match app.selected_task_index() {
        Some(idx) => idx,
        None => return,
    };

    let task_id = match app.tasks.get(task_idx) {
        Some(t) => t.id.clone(),
        None => return,
    };

    app.set_loading(true);
    match auth.access_token().await {
        Ok(token) => match client.delete_task(&token, &tasklist_id, &task_id).await {
            Ok(_) => {
                app.set_status("Task deleted");
                refresh_tasks(app, auth, client).await;
            }
            Err(e) => app.set_status(format!("Delete failed: {e}")),
        },
        Err(e) => app.set_status(format!("Auth failed: {e}")),
    }
    app.set_loading(false);
}
