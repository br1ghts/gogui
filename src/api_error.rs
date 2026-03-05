use reqwest::StatusCode;
use serde::Deserialize;

#[derive(Debug, Clone, thiserror::Error)]
pub enum ApiError {
    #[error("authentication expired (401)")]
    AuthExpired,
    #[error("Google API not enabled")]
    ApiNotEnabled,
    #[error("missing required scope: {0}")]
    MissingScope(String),
    #[error("rate limited (429)")]
    RateLimited,
    #[error("transient server error ({0})")]
    Transient(u16),
    #[error("http {status}: {message}")]
    Http { status: u16, message: String },
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Deserialize)]
struct GoogleEnvelope {
    error: GoogleError,
}

#[derive(Debug, Deserialize)]
struct GoogleError {
    message: Option<String>,
    errors: Option<Vec<GoogleReason>>,
}

#[derive(Debug, Deserialize)]
struct GoogleReason {
    reason: Option<String>,
}

pub fn map_http_error(status: StatusCode, body: &str) -> ApiError {
    if status == StatusCode::UNAUTHORIZED {
        return ApiError::AuthExpired;
    }

    if status == StatusCode::TOO_MANY_REQUESTS {
        return ApiError::RateLimited;
    }

    if status.is_server_error() {
        return ApiError::Transient(status.as_u16());
    }

    let parsed = serde_json::from_str::<GoogleEnvelope>(body).ok();
    let reason = parsed
        .as_ref()
        .and_then(|e| e.error.errors.as_ref())
        .and_then(|errs| errs.first())
        .and_then(|r| r.reason.as_deref());

    if status == StatusCode::FORBIDDEN {
        if reason == Some("accessNotConfigured") {
            return ApiError::ApiNotEnabled;
        }
        if reason == Some("insufficientPermissions") {
            return ApiError::MissingScope("unknown".to_string());
        }
    }

    let message = parsed
        .and_then(|e| e.error.message)
        .unwrap_or_else(|| body.to_string());

    ApiError::Http {
        status: status.as_u16(),
        message,
    }
}

pub fn actionable_message(err: &ApiError) -> String {
    match err {
        ApiError::ApiNotEnabled => "Enable Google Calendar API in Google Cloud Console".to_string(),
        ApiError::MissingScope(scope) if scope == "calendar.events" => {
            "Re-auth required: missing scope calendar.events".to_string()
        }
        ApiError::MissingScope(scope) => format!("Re-auth required: missing scope {scope}"),
        ApiError::AuthExpired => "Authentication expired. Re-authenticate.".to_string(),
        ApiError::RateLimited => "Rate limited by API. Try again shortly.".to_string(),
        ApiError::Transient(_) => "Google API transient error. Retry.".to_string(),
        ApiError::Http { status, message } => format!("API error {status}: {message}"),
        ApiError::Other(msg) => msg.clone(),
    }
}
