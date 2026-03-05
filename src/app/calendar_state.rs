use chrono::{Duration, Utc};

use crate::calendar::models::{CalendarEvent, CalendarItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarPane {
    Calendars,
    Agenda,
    Detail,
}

impl Default for CalendarPane {
    fn default() -> Self {
        Self::Calendars
    }
}

#[derive(Debug, Default, Clone)]
pub struct CalendarState {
    pub calendars: Vec<CalendarItem>,
    pub events: Vec<CalendarEvent>,
    pub selected_calendar: usize,
    pub selected_event: usize,
    pub active_pane: CalendarPane,
    pub search_query: String,
    pub range_start: chrono::DateTime<Utc>,
    pub range_days: i64,
}

impl CalendarState {
    pub fn new() -> Self {
        Self {
            calendars: Vec::new(),
            events: Vec::new(),
            selected_calendar: 0,
            selected_event: 0,
            active_pane: CalendarPane::Calendars,
            search_query: String::new(),
            range_start: Utc::now(),
            range_days: 14,
        }
    }

    pub fn range_end(&self) -> chrono::DateTime<Utc> {
        self.range_start + Duration::days(self.range_days)
    }

    pub fn selected_calendar_id(&self) -> Option<&str> {
        self.calendars.get(self.selected_calendar).map(|c| c.id.as_str())
    }

    pub fn selected_event(&self) -> Option<&CalendarEvent> {
        self.filtered_events().get(self.selected_event).map(|(_, e)| *e)
    }

    pub fn filtered_events(&self) -> Vec<(usize, &CalendarEvent)> {
        let q = self.search_query.to_lowercase();
        self.events
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                q.is_empty()
                    || e.title.to_lowercase().contains(&q)
                    || e.location.as_deref().unwrap_or_default().to_lowercase().contains(&q)
                    || e.description.as_deref().unwrap_or_default().to_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn set_calendars(&mut self, mut calendars: Vec<CalendarItem>) {
        calendars.sort_by(|a, b| b.primary.cmp(&a.primary).then(a.summary.cmp(&b.summary)));
        self.calendars = calendars;
        if self.selected_calendar >= self.calendars.len() {
            self.selected_calendar = self.calendars.len().saturating_sub(1);
        }
    }

    pub fn set_events(&mut self, events: Vec<CalendarEvent>) {
        self.events = events;
        let len = self.filtered_events().len();
        if self.selected_event >= len {
            self.selected_event = len.saturating_sub(1);
        }
    }

    pub fn pane_next(&mut self) {
        self.active_pane = match self.active_pane {
            CalendarPane::Calendars => CalendarPane::Agenda,
            CalendarPane::Agenda => CalendarPane::Detail,
            CalendarPane::Detail => CalendarPane::Calendars,
        }
    }

    pub fn pane_prev(&mut self) {
        self.active_pane = match self.active_pane {
            CalendarPane::Calendars => CalendarPane::Detail,
            CalendarPane::Agenda => CalendarPane::Calendars,
            CalendarPane::Detail => CalendarPane::Agenda,
        }
    }

    pub fn move_up(&mut self) {
        match self.active_pane {
            CalendarPane::Calendars => {
                if self.selected_calendar > 0 {
                    self.selected_calendar -= 1;
                    self.selected_event = 0;
                }
            }
            CalendarPane::Agenda => {
                if self.selected_event > 0 {
                    self.selected_event -= 1;
                }
            }
            CalendarPane::Detail => {}
        }
    }

    pub fn move_down(&mut self) {
        match self.active_pane {
            CalendarPane::Calendars => {
                if self.selected_calendar + 1 < self.calendars.len() {
                    self.selected_calendar += 1;
                    self.selected_event = 0;
                }
            }
            CalendarPane::Agenda => {
                if self.selected_event + 1 < self.filtered_events().len() {
                    self.selected_event += 1;
                }
            }
            CalendarPane::Detail => {}
        }
    }

    pub fn jump_today(&mut self) {
        self.range_start = Utc::now();
    }

    pub fn shift_range(&mut self, days: i64) {
        self.range_start += Duration::days(days);
    }
}
