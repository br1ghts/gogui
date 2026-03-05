use chrono::{DateTime, NaiveDate};

use crate::api_error::ApiError;
use crate::calendar::api::{EventInsertRequest, EventWriteDateTime};
use crate::calendar::models::EventEdit;

pub fn build_event_request(edit: &EventEdit) -> Result<EventInsertRequest, ApiError> {
    if edit.title.trim().is_empty() {
        return Err(ApiError::Other("Title is required".to_string()));
    }

    if edit.all_day {
        let start = NaiveDate::parse_from_str(edit.start.trim(), "%Y-%m-%d")
            .map_err(|_| ApiError::Other("All-day start must be YYYY-MM-DD".to_string()))?;
        let end = NaiveDate::parse_from_str(edit.end.trim(), "%Y-%m-%d")
            .map_err(|_| ApiError::Other("All-day end must be YYYY-MM-DD".to_string()))?;
        Ok(EventInsertRequest {
            summary: edit.title.trim().to_string(),
            start: EventWriteDateTime {
                date_time: None,
                date: Some(start.to_string()),
                time_zone: None,
            },
            end: EventWriteDateTime {
                date_time: None,
                date: Some(end.to_string()),
                time_zone: None,
            },
            location: empty_to_none(&edit.location),
            description: empty_to_none(&edit.description),
        })
    } else {
        let start = DateTime::parse_from_rfc3339(edit.start.trim())
            .map_err(|_| ApiError::Other("Start must be RFC3339".to_string()))?;
        let end = DateTime::parse_from_rfc3339(edit.end.trim())
            .map_err(|_| ApiError::Other("End must be RFC3339".to_string()))?;
        Ok(EventInsertRequest {
            summary: edit.title.trim().to_string(),
            start: EventWriteDateTime {
                date_time: Some(start.to_rfc3339()),
                date: None,
                time_zone: Some(start.offset().to_string()),
            },
            end: EventWriteDateTime {
                date_time: Some(end.to_rfc3339()),
                date: None,
                time_zone: Some(end.offset().to_string()),
            },
            location: empty_to_none(&edit.location),
            description: empty_to_none(&edit.description),
        })
    }
}

fn empty_to_none(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
