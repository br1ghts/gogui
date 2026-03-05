pub mod gmail_state;
pub mod calendar_state;

use crate::gmail::models::MessageDetail;
use crate::modal::{EditorField, EditorMode, ModalState};
use crate::models::{Task, TaskList};
use crate::workspace::ModuleKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Tasks,
    Gmail,
    Calendar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    TaskLists,
    Tasks,
    Details,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiMode {
    CommandCenter,
    Focused(ModuleKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardTile {
    Tasks,
    Calendar,
    Gmail,
}

#[derive(Debug, Clone)]
pub struct FocusState {
    pub mode: UiMode,
    pub focused_tile: DashboardTile,
}

#[derive(Debug)]
pub struct App {
    pub account_label: String,
    pub active_tab: ActiveTab,
    pub tasklists: Vec<TaskList>,
    pub tasks: Vec<Task>,
    pub selected_tasklist: usize,
    pub selected_task_view: usize,
    pub active_pane: ActivePane,
    pub show_completed: bool,
    pub search_query: String,
    pub status: String,
    pub loading: bool,
    pub spinner_idx: usize,
    pub modal: Option<ModalState>,
    pub should_quit: bool,
    pub show_help: bool,
    pub gmail: gmail_state::GmailState,
    pub calendar: calendar_state::CalendarState,
    pub focus: FocusState,
    pub command_palette_open: bool,
    pub command_palette: String,
}

impl App {
    pub fn new(account_label: String) -> Self {
        Self {
            account_label,
            active_tab: ActiveTab::Tasks,
            tasklists: Vec::new(),
            tasks: Vec::new(),
            selected_tasklist: 0,
            selected_task_view: 0,
            active_pane: ActivePane::TaskLists,
            show_completed: false,
            search_query: String::new(),
            status: "Press g to refresh, q to quit".to_string(),
            loading: false,
            spinner_idx: 0,
            modal: None,
            should_quit: false,
            show_help: false,
            gmail: gmail_state::GmailState::default(),
            calendar: calendar_state::CalendarState::new(),
            focus: FocusState {
                mode: UiMode::CommandCenter,
                focused_tile: DashboardTile::Tasks,
            },
            command_palette_open: false,
            command_palette: String::new(),
        }
    }

    pub fn spinner_char(&self) -> char {
        const FRAMES: [char; 4] = ['|', '/', '-', '\\'];
        FRAMES[self.spinner_idx % FRAMES.len()]
    }

    pub fn tick(&mut self) {
        self.spinner_idx = (self.spinner_idx + 1) % 4;
    }

    pub fn set_status<S: Into<String>>(&mut self, s: S) {
        self.status = s.into();
    }

    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    pub fn switch_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Tasks => ActiveTab::Gmail,
            ActiveTab::Gmail => ActiveTab::Calendar,
            ActiveTab::Calendar => ActiveTab::Tasks,
        };
        self.focus.mode = match self.active_tab {
            ActiveTab::Tasks => UiMode::Focused(ModuleKind::Tasks),
            ActiveTab::Gmail => UiMode::Focused(ModuleKind::Gmail),
            ActiveTab::Calendar => UiMode::Focused(ModuleKind::Calendar),
        };
    }

    pub fn set_tab(&mut self, tab: ActiveTab) {
        self.active_tab = tab;
        self.focus.mode = match self.active_tab {
            ActiveTab::Tasks => UiMode::Focused(ModuleKind::Tasks),
            ActiveTab::Gmail => UiMode::Focused(ModuleKind::Gmail),
            ActiveTab::Calendar => UiMode::Focused(ModuleKind::Calendar),
        };
    }

    pub fn toggle_dashboard(&mut self) {
        self.focus.mode = match self.focus.mode {
            UiMode::Focused(_) => UiMode::CommandCenter,
            UiMode::CommandCenter => match self.active_tab {
                ActiveTab::Tasks => UiMode::Focused(ModuleKind::Tasks),
                ActiveTab::Gmail => UiMode::Focused(ModuleKind::Gmail),
                ActiveTab::Calendar => UiMode::Focused(ModuleKind::Calendar),
            },
        };
    }

    pub fn cycle_tile(&mut self, backwards: bool) {
        self.focus.focused_tile = if backwards {
            match self.focus.focused_tile {
                DashboardTile::Tasks => DashboardTile::Gmail,
                DashboardTile::Calendar => DashboardTile::Tasks,
                DashboardTile::Gmail => DashboardTile::Calendar,
            }
        } else {
            match self.focus.focused_tile {
                DashboardTile::Tasks => DashboardTile::Calendar,
                DashboardTile::Calendar => DashboardTile::Gmail,
                DashboardTile::Gmail => DashboardTile::Tasks,
            }
        };
    }

    pub fn set_gmail_detail(&mut self, detail: Option<MessageDetail>) {
        self.gmail.detail = detail;
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn set_tasklists(&mut self, lists: Vec<TaskList>) {
        self.tasklists = lists;
        if self.selected_tasklist >= self.tasklists.len() {
            self.selected_tasklist = self.tasklists.len().saturating_sub(1);
        }
        self.clamp_task_selection();
    }

    pub fn set_tasks(&mut self, tasks: Vec<Task>) {
        self.tasks = tasks;
        self.clamp_task_selection();
    }

    pub fn filtered_task_indices(&self) -> Vec<usize> {
        let q = self.search_query.to_lowercase();
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| self.show_completed || !t.is_completed())
            .filter(|(_, t)| {
                if q.is_empty() {
                    true
                } else {
                    t.title.to_lowercase().contains(&q)
                        || t
                            .notes
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&q)
                }
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn selected_task_index(&self) -> Option<usize> {
        let filtered = self.filtered_task_indices();
        filtered.get(self.selected_task_view).copied()
    }

    pub fn selected_tasklist_id(&self) -> Option<&str> {
        self.tasklists.get(self.selected_tasklist).map(|l| l.id.as_str())
    }

    pub fn selected_tasklist_title(&self) -> String {
        self.tasklists
            .get(self.selected_tasklist)
            .map(|l| l.title.clone())
            .unwrap_or_else(|| "No tasklist".to_string())
    }

    pub fn selected_task(&self) -> Option<&Task> {
        self.selected_task_index().and_then(|i| self.tasks.get(i))
    }

    pub fn move_up(&mut self) {
        match self.active_pane {
            ActivePane::TaskLists => {
                if self.selected_tasklist > 0 {
                    self.selected_tasklist -= 1;
                    self.selected_task_view = 0;
                }
            }
            ActivePane::Tasks => {
                if self.selected_task_view > 0 {
                    self.selected_task_view -= 1;
                }
            }
            ActivePane::Details => {}
        }
    }

    pub fn move_down(&mut self) {
        match self.active_pane {
            ActivePane::TaskLists => {
                if self.selected_tasklist + 1 < self.tasklists.len() {
                    self.selected_tasklist += 1;
                    self.selected_task_view = 0;
                }
            }
            ActivePane::Tasks => {
                let len = self.filtered_task_indices().len();
                if self.selected_task_view + 1 < len {
                    self.selected_task_view += 1;
                }
            }
            ActivePane::Details => {}
        }
    }

    pub fn pane_right(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::TaskLists => ActivePane::Tasks,
            ActivePane::Tasks => ActivePane::Details,
            ActivePane::Details => ActivePane::TaskLists,
        };
    }

    pub fn focus_details(&mut self) {
        self.active_pane = ActivePane::Details;
    }

    pub fn toggle_completed_filter(&mut self) {
        self.show_completed = !self.show_completed;
        self.clamp_task_selection();
    }

    pub fn open_add_modal(&mut self) {
        self.modal = Some(ModalState::TaskEditor {
            mode: EditorMode::Add,
            title: String::new(),
            notes: String::new(),
            field: EditorField::Title,
            quick: false,
        });
    }

    pub fn open_add_modal_quick(&mut self) {
        self.modal = Some(ModalState::TaskEditor {
            mode: EditorMode::Add,
            title: String::new(),
            notes: String::new(),
            field: EditorField::Title,
            quick: true,
        });
    }

    pub fn open_edit_modal(&mut self) {
        if let Some(task) = self.selected_task() {
            self.modal = Some(ModalState::TaskEditor {
                mode: EditorMode::Edit,
                title: task.title.clone(),
                notes: task.notes.clone().unwrap_or_default(),
                field: EditorField::Title,
                quick: false,
            });
        }
    }

    pub fn open_edit_modal_quick(&mut self) {
        if let Some(task) = self.selected_task() {
            self.modal = Some(ModalState::TaskEditor {
                mode: EditorMode::Edit,
                title: task.title.clone(),
                notes: task.notes.clone().unwrap_or_default(),
                field: EditorField::Title,
                quick: true,
            });
        }
    }

    pub fn open_delete_modal(&mut self) {
        if self.selected_task().is_some() {
            self.modal = Some(ModalState::ConfirmDelete);
        }
    }

    pub fn open_search_modal(&mut self) {
        self.modal = Some(ModalState::Search {
            query: self.search_query.clone(),
        });
    }

    pub fn close_modal(&mut self) {
        self.modal = None;
    }

    pub fn apply_search(&mut self, query: String) {
        self.search_query = query;
        self.selected_task_view = 0;
        self.clamp_task_selection();
    }

    fn clamp_task_selection(&mut self) {
        let len = self.filtered_task_indices().len();
        if len == 0 {
            self.selected_task_view = 0;
        } else if self.selected_task_view >= len {
            self.selected_task_view = len - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Task;

    #[test]
    fn tab_cycle_order_is_workspace_order() {
        let mut app = App::new("x".to_string());
        app.focus.mode = UiMode::CommandCenter;
        app.focus.focused_tile = DashboardTile::Tasks;
        app.cycle_tile(false);
        assert_eq!(app.focus.focused_tile, DashboardTile::Calendar);
        app.cycle_tile(false);
        assert_eq!(app.focus.focused_tile, DashboardTile::Gmail);
        app.cycle_tile(false);
        assert_eq!(app.focus.focused_tile, DashboardTile::Tasks);
    }

    #[test]
    fn mode_switching_enter_to_full_and_esc_back() {
        let mut app = App::new("x".to_string());
        app.focus.mode = UiMode::CommandCenter;
        app.focus.focused_tile = DashboardTile::Calendar;
        app.focus.mode = UiMode::Focused(ModuleKind::Calendar);
        assert!(matches!(app.focus.mode, UiMode::Focused(ModuleKind::Calendar)));
        app.focus.mode = UiMode::CommandCenter;
        assert!(matches!(app.focus.mode, UiMode::CommandCenter));
    }

    #[test]
    fn selection_persists_across_mode_switch() {
        let mut app = App::new("x".to_string());
        app.tasks = vec![
            Task { id: "1".into(), title: "A".into(), notes: None, due: None, status: Some("needsAction".into()), updated: None, completed: None, deleted: None, hidden: None },
            Task { id: "2".into(), title: "B".into(), notes: None, due: None, status: Some("needsAction".into()), updated: None, completed: None, deleted: None, hidden: None },
        ];
        app.selected_task_view = 1;
        app.focus.mode = UiMode::Focused(ModuleKind::Tasks);
        app.focus.mode = UiMode::CommandCenter;
        app.focus.mode = UiMode::Focused(ModuleKind::Tasks);
        assert_eq!(app.selected_task_view, 1);
    }
}
