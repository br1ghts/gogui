use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    None,
    Quit,
    MoveUp,
    MoveDown,
    NextPane,
    Refresh,
    ToggleShowCompleted,
    OpenAdd,
    OpenEdit,
    ToggleComplete,
    OpenDelete,
    OpenSearch,
    CancelModal,
    SwitchTab,
    GmailArchive,
    GmailUnread,
    GmailReply,
    GmailCompose,
    ToggleHelp,
    PrevPane,
    CalendarNew,
    CalendarToday,
    CalendarRangeBack,
    CalendarRangeForward,
    ToggleDashboard,
    RefreshVisible,
    NextTile,
    PrevTile,
    ZoomTile,
    LoadTabTasks,
    LoadTabGmail,
    LoadTabCalendar,
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    AdjustSplitLeft,
    AdjustSplitRight,
    AdjustSplitUp,
    AdjustSplitDown,
    ExitZoom,
    OpenCommandPalette,
}

pub fn action_from_key(key: KeyEvent, modal_open: bool) -> AppAction {
    if modal_open {
        if key.code == KeyCode::Esc {
            return AppAction::CancelModal;
        }
        return AppAction::None;
    }

    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) => AppAction::Quit,
        (KeyCode::Char('D'), _) => AppAction::ToggleDashboard,
        (KeyCode::Char('W'), _) => AppAction::None,
        (KeyCode::Char('r'), KeyModifiers::CONTROL) => AppAction::RefreshVisible,
        (KeyCode::Char('R'), KeyModifiers::CONTROL | KeyModifiers::SHIFT) => AppAction::RefreshVisible,
        (KeyCode::Char('h'), KeyModifiers::CONTROL) => AppAction::FocusLeft,
        (KeyCode::Char('j'), KeyModifiers::CONTROL) => AppAction::FocusDown,
        (KeyCode::Char('k'), KeyModifiers::CONTROL) => AppAction::FocusUp,
        (KeyCode::Char('l'), KeyModifiers::CONTROL) => AppAction::FocusRight,
        (KeyCode::Left, KeyModifiers::ALT) => AppAction::AdjustSplitLeft,
        (KeyCode::Right, KeyModifiers::ALT) => AppAction::AdjustSplitRight,
        (KeyCode::Up, KeyModifiers::ALT) => AppAction::AdjustSplitUp,
        (KeyCode::Down, KeyModifiers::ALT) => AppAction::AdjustSplitDown,
        (KeyCode::BackTab, _) => AppAction::PrevTile,
        (KeyCode::Char('1'), KeyModifiers::CONTROL) => AppAction::LoadTabTasks,
        (KeyCode::Char('2'), KeyModifiers::CONTROL) => AppAction::LoadTabGmail,
        (KeyCode::Char('3'), KeyModifiers::CONTROL) => AppAction::LoadTabCalendar,
        (KeyCode::Char('T'), _) => AppAction::SwitchTab,
        (KeyCode::Char('?'), _) => AppAction::ToggleHelp,
        (KeyCode::Char('j'), _) | (KeyCode::Down, _) => AppAction::MoveDown,
        (KeyCode::Char('k'), _) | (KeyCode::Up, _) => AppAction::MoveUp,
        (KeyCode::Tab, _) => AppAction::NextTile,
        (KeyCode::Char('h'), _) | (KeyCode::Left, _) => AppAction::PrevPane,
        (KeyCode::Char('l'), _) | (KeyCode::Right, _) => AppAction::NextPane,
        (KeyCode::Enter, _) => AppAction::ZoomTile,
        (KeyCode::Esc, _) => AppAction::ExitZoom,
        (KeyCode::Char(':'), _) => AppAction::OpenCommandPalette,
        (KeyCode::Char('g'), _) => AppAction::Refresh,
        (KeyCode::Char('a'), _) => AppAction::OpenAdd,
        (KeyCode::Char('e'), _) => AppAction::OpenEdit,
        (KeyCode::Char('x'), _) => AppAction::ToggleComplete,
        (KeyCode::Char('d'), _) => AppAction::OpenDelete,
        (KeyCode::Char('/'), _) => AppAction::OpenSearch,
        (KeyCode::Char('c'), KeyModifiers::NONE) => AppAction::ToggleShowCompleted,
        (KeyCode::Char('u'), _) => AppAction::GmailUnread,
        (KeyCode::Char('r'), _) => AppAction::GmailReply,
        (KeyCode::Char('A'), _) => AppAction::GmailArchive,
        (KeyCode::Char('n'), _) => AppAction::CalendarNew,
        (KeyCode::Char('t'), _) => AppAction::CalendarToday,
        (KeyCode::Char(']'), _) => AppAction::CalendarRangeForward,
        (KeyCode::Char('['), _) => AppAction::CalendarRangeBack,
        _ => AppAction::None,
    }
}

pub fn gmail_action_from_key(key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Char('a') => AppAction::GmailArchive,
        KeyCode::Char('u') => AppAction::GmailUnread,
        KeyCode::Char('r') => AppAction::GmailReply,
        KeyCode::Char('c') => AppAction::GmailCompose,
        _ => action_from_key(key, false),
    }
}
