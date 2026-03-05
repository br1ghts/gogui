use chrono::{DateTime, NaiveDate};

#[derive(Debug, Clone)]
pub struct CalendarItem {
    pub id: String,
    pub summary: String,
    pub primary: bool,
}

#[derive(Debug, Clone)]
pub enum EventTime {
    DateTime(DateTime<chrono::FixedOffset>),
    AllDay(NaiveDate),
}

#[derive(Debug, Clone)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub start: EventTime,
    pub end: EventTime,
    pub location: Option<String>,
    pub description: Option<String>,
    pub transparency: Option<String>,
    pub attendees_count: usize,
    pub meet_link: Option<String>,
    pub timezone: Option<String>,
}

impl CalendarEvent {
    pub fn is_all_day(&self) -> bool {
        matches!(self.start, EventTime::AllDay(_))
    }

    pub fn is_free(&self) -> bool {
        self.transparency.as_deref() == Some("transparent")
    }
}

#[derive(Debug, Clone)]
pub struct EventEdit {
    pub title: String,
    pub start: String,
    pub end: String,
    pub all_day: bool,
    pub location: String,
    pub description: String,
}
