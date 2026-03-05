use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CalendarListResponse {
    pub items: Option<Vec<CalendarDto>>,
}

#[derive(Debug, Deserialize)]
pub struct CalendarDto {
    pub id: String,
    pub summary: Option<String>,
    pub primary: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct EventsResponse {
    pub items: Option<Vec<EventDto>>,
}

#[derive(Debug, Deserialize)]
pub struct EventDto {
    pub id: String,
    pub summary: Option<String>,
    pub start: EventDateTime,
    pub end: EventDateTime,
    pub location: Option<String>,
    pub description: Option<String>,
    pub transparency: Option<String>,
    pub attendees: Option<Vec<serde_json::Value>>,
    #[serde(rename = "hangoutLink")]
    pub hangout_link: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EventDateTime {
    #[serde(rename = "dateTime")]
    pub date_time: Option<String>,
    pub date: Option<String>,
    #[serde(rename = "timeZone")]
    pub time_zone: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EventInsertRequest {
    pub summary: String,
    pub start: EventWriteDateTime,
    pub end: EventWriteDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EventWriteDateTime {
    #[serde(rename = "dateTime", skip_serializing_if = "Option::is_none")]
    pub date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(rename = "timeZone", skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
}
