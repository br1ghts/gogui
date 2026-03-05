use std::collections::HashSet;
use std::time::{Duration, Instant};

use crossterm::event::KeyEvent;

use crate::app::{App, DashboardTile, UiMode};
use crate::input::{action_from_key, gmail_action_from_key, AppAction};
use crate::modal::ModalSubmit;
use crate::workspace::ModuleKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutedModule {
    Tasks,
    Gmail,
    Calendar,
}

pub fn routed_module(app: &App) -> RoutedModule {
    match app.focus.mode {
        UiMode::CommandCenter => match app.focus.focused_tile {
            DashboardTile::Tasks => RoutedModule::Tasks,
            DashboardTile::Calendar => RoutedModule::Calendar,
            DashboardTile::Gmail => RoutedModule::Gmail,
        },
        UiMode::Focused(ModuleKind::Tasks) => RoutedModule::Tasks,
        UiMode::Focused(ModuleKind::Gmail) => RoutedModule::Gmail,
        UiMode::Focused(ModuleKind::Calendar) => RoutedModule::Calendar,
    }
}

#[derive(Debug, Clone)]
pub enum Command {}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RouteOutcome {
    pub handled: bool,
    pub actions: Vec<AppAction>,
    pub maybe_command: Vec<Command>,
    pub modal_submit: Option<ModalSubmit>,
}

pub fn route_key(app: &mut App, key: KeyEvent) -> RouteOutcome {
    if let Some(modal) = app.modal.as_mut() {
        let action = action_from_key(key, true);
        if action == AppAction::CancelModal {
            return RouteOutcome {
                handled: true,
                actions: vec![AppAction::CancelModal],
                maybe_command: Vec::new(),
                modal_submit: None,
            };
        }
        let submit = modal.handle_key(key);
        return RouteOutcome {
            handled: true,
            actions: Vec::new(),
            maybe_command: Vec::new(),
            modal_submit: submit,
        };
    }

    let action = route_key_to_action(app, key);
    RouteOutcome {
        handled: action != AppAction::None,
        actions: vec![action],
        maybe_command: Vec::new(),
        modal_submit: None,
    }
}

pub fn route_key_to_action(app: &App, key: KeyEvent) -> AppAction {
    match routed_module(app) {
        RoutedModule::Gmail => gmail_action_from_key(key),
        RoutedModule::Tasks | RoutedModule::Calendar => action_from_key(key, false),
    }
}

#[derive(Debug)]
pub struct RefreshGate {
    in_flight: HashSet<String>,
    last_run: Option<Instant>,
    min_gap: Duration,
}

impl RefreshGate {
    pub fn new(min_gap: Duration) -> Self {
        Self {
            in_flight: HashSet::new(),
            last_run: None,
            min_gap,
        }
    }

    pub fn try_start(&mut self, key: &str) -> bool {
        if self.in_flight.contains(key) {
            return false;
        }
        if let Some(last) = self.last_run {
            if last.elapsed() < self.min_gap {
                return false;
            }
        }
        self.in_flight.insert(key.to_string());
        self.last_run = Some(Instant::now());
        true
    }

    pub fn finish(&mut self, key: &str) {
        self.in_flight.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;

    #[test]
    fn routes_to_active_dashboard_tile() {
        let mut app = App::new("x".to_string());
        app.focus.mode = UiMode::CommandCenter;
        app.focus.focused_tile = DashboardTile::Gmail;
        assert_eq!(routed_module(&app), RoutedModule::Gmail);
    }

    #[test]
    fn routes_to_focused_tab_when_not_dashboard() {
        let mut app = App::new("x".to_string());
        app.focus.mode = UiMode::Focused(ModuleKind::Calendar);
        assert_eq!(routed_module(&app), RoutedModule::Calendar);
    }

    #[test]
    fn zoom_routing_precedence() {
        let mut app = App::new("x".to_string());
        app.focus.mode = UiMode::CommandCenter;
        app.focus.focused_tile = DashboardTile::Calendar;
        assert_eq!(routed_module(&app), RoutedModule::Calendar);
    }

    #[test]
    fn focused_tile_key_routes_to_module_action() {
        let mut app = App::new("x".to_string());
        app.focus.mode = UiMode::CommandCenter;
        app.focus.focused_tile = DashboardTile::Gmail;
        let key = crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Char('a'));
        let action = route_key_to_action(&app, key);
        assert_eq!(action, AppAction::GmailArchive);
    }

    #[test]
    fn dedupes_and_debounces() {
        let mut g = RefreshGate::new(Duration::from_millis(300));
        assert!(g.try_start("gmail:threads"));
        assert!(!g.try_start("gmail:threads"));
        g.finish("gmail:threads");
        assert!(!g.try_start("gmail:threads"));
    }
}
