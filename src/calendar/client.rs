use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    RefreshToken, Scope, TokenResponse, TokenUrl,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::api_error::{map_http_error, ApiError};
use crate::calendar::api::{CalendarListResponse, EventDto, EventInsertRequest, EventsResponse};
use crate::calendar::models::{CalendarEvent, CalendarItem, EventTime};
use crate::storage;

const READ_SCOPE: &str = "https://www.googleapis.com/auth/calendar.readonly";
const WRITE_SCOPE: &str = "https://www.googleapis.com/auth/calendar.events";

#[derive(Debug, Deserialize)]
struct CredentialsFile {
    installed: InstalledCredentials,
}

#[derive(Debug, Deserialize)]
struct InstalledCredentials {
    client_id: String,
    client_secret: Option<String>,
    auth_uri: String,
    token_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredToken {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
}

pub struct CalendarClient {
    http: Client,
    oauth_client: BasicClient,
    token: StoredToken,
    base_url: String,
    tokeninfo_url: String,
}

impl CalendarClient {
    pub async fn from_disk_or_authorize() -> Result<Self, ApiError> {
        storage::ensure_config_dir().map_err(ApiError::Other)?;
        let creds_path = storage::credentials_path().map_err(ApiError::Other)?;
        let creds: CredentialsFile = storage::read_json(&creds_path).map_err(ApiError::Other)?;

        let auth_url = AuthUrl::new(creds.installed.auth_uri.clone()).map_err(|e| ApiError::Other(e.to_string()))?;
        let token_url = TokenUrl::new(creds.installed.token_uri.clone()).map_err(|e| ApiError::Other(e.to_string()))?;

        let oauth_client = BasicClient::new(
            ClientId::new(creds.installed.client_id.clone()),
            Some(ClientSecret::new(creds.installed.client_secret.clone().unwrap_or_default())),
            auth_url,
            Some(token_url),
        );

        let token_path = storage::calendar_token_path().map_err(ApiError::Other)?;
        let token = if token_path.exists() {
            storage::read_json(&token_path).map_err(ApiError::Other)?
        } else {
            authorize_interactive(&creds).await?
        };

        Ok(Self {
            http: Client::new(),
            oauth_client,
            token,
            base_url: "https://www.googleapis.com".to_string(),
            tokeninfo_url: "https://oauth2.googleapis.com/tokeninfo".to_string(),
        })
    }

    #[cfg(test)]
    pub fn with_base_url_and_token(base_url: String, token: String) -> Self {
        let tokeninfo_url = format!("{base_url}/tokeninfo");
        let oauth_client = BasicClient::new(
            ClientId::new("test".to_string()),
            Some(ClientSecret::new("test".to_string())),
            AuthUrl::new("http://localhost/auth".to_string()).expect("valid auth url"),
            Some(TokenUrl::new("http://localhost/token".to_string()).expect("valid token url")),
        );
        Self {
            http: Client::new(),
            oauth_client,
            token: StoredToken {
                access_token: token,
                refresh_token: "rt".to_string(),
                expires_at: now_unix() + 3600,
            },
            base_url,
            tokeninfo_url,
        }
    }

    pub async fn preflight_read(&mut self) -> Result<(), ApiError> {
        let _ = self.access_token().await?;
        self.list_calendars().await.map(|_| ())
    }

    pub async fn preflight_write(&mut self) -> Result<(), ApiError> {
        let token = self.access_token().await?;
        let scopes = self.fetch_token_scopes(&token).await?;
        if !scopes.iter().any(|s| s == WRITE_SCOPE) {
            return Err(ApiError::MissingScope("calendar.events".to_string()));
        }
        Ok(())
    }

    async fn access_token(&mut self) -> Result<String, ApiError> {
        if self.token.expires_at - 60 > now_unix() {
            return Ok(self.token.access_token.clone());
        }

        let token_res = self
            .oauth_client
            .exchange_refresh_token(&RefreshToken::new(self.token.refresh_token.clone()))
            .request_async(async_http_client)
            .await
            .map_err(|e| ApiError::Other(format!("Refresh failed: {e}")))?;

        self.token.access_token = token_res.access_token().secret().to_string();
        self.token.expires_at = now_unix() + token_res.expires_in().unwrap_or(Duration::from_secs(3600)).as_secs() as i64;
        if let Some(rt) = token_res.refresh_token() {
            self.token.refresh_token = rt.secret().to_string();
        }
        let token_path = storage::calendar_token_path().map_err(ApiError::Other)?;
        storage::write_json(&token_path, &self.token).map_err(ApiError::Other)?;
        Ok(self.token.access_token.clone())
    }

    async fn fetch_token_scopes(&self, access_token: &str) -> Result<Vec<String>, ApiError> {
        let resp = self
            .http
            .get(&self.tokeninfo_url)
            .query(&[("access_token", access_token)])
            .send()
            .await
            .map_err(|e| ApiError::Other(format!("tokeninfo failed: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }
        let v: serde_json::Value = resp.json().await.map_err(|e| ApiError::Other(e.to_string()))?;
        Ok(v
            .get("scope")
            .and_then(|s| s.as_str())
            .unwrap_or_default()
            .split(' ')
            .map(|s| s.to_string())
            .collect())
    }

    pub async fn list_calendars(&mut self) -> Result<Vec<CalendarItem>, ApiError> {
        let token = self.access_token().await?;
        let resp = self
            .http
            .get(format!("{}/calendar/v3/users/me/calendarList", self.base_url))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| ApiError::Other(format!("calendar list failed: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }
        let dto: CalendarListResponse = resp.json().await.map_err(|e| ApiError::Other(e.to_string()))?;
        Ok(dto
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|c| CalendarItem {
                id: c.id,
                summary: c.summary.unwrap_or_else(|| "Untitled".to_string()),
                primary: c.primary.unwrap_or(false),
            })
            .collect())
    }

    pub async fn list_events(
        &mut self,
        calendar_id: &str,
        time_min: DateTime<Utc>,
        time_max: DateTime<Utc>,
    ) -> Result<Vec<CalendarEvent>, ApiError> {
        let token = self.access_token().await?;
        let resp = self
            .http
            .get(format!("{}/calendar/v3/calendars/{}/events", self.base_url, urlencoding::encode(calendar_id)))
            .query(&[
                ("singleEvents", "true"),
                ("orderBy", "startTime"),
                ("timeMin", &time_min.to_rfc3339()),
                ("timeMax", &time_max.to_rfc3339()),
            ])
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| ApiError::Other(format!("events list failed: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }
        let dto: EventsResponse = resp.json().await.map_err(|e| ApiError::Other(e.to_string()))?;
        dto.items
            .unwrap_or_default()
            .into_iter()
            .map(|e| map_event(calendar_id, e))
            .collect()
    }

    pub async fn insert_event(&mut self, calendar_id: &str, req: &EventInsertRequest) -> Result<(), ApiError> {
        self.preflight_write().await?;
        let token = self.access_token().await?;
        let resp = self
            .http
            .post(format!("{}/calendar/v3/calendars/{}/events", self.base_url, urlencoding::encode(calendar_id)))
            .bearer_auth(token)
            .json(req)
            .send()
            .await
            .map_err(|e| ApiError::Other(format!("insert failed: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }
        Ok(())
    }

    pub async fn patch_event(&mut self, calendar_id: &str, event_id: &str, req: &EventInsertRequest) -> Result<(), ApiError> {
        self.preflight_write().await?;
        let token = self.access_token().await?;
        let resp = self
            .http
            .patch(format!(
                "{}/calendar/v3/calendars/{}/events/{}",
                self.base_url,
                urlencoding::encode(calendar_id),
                urlencoding::encode(event_id)
            ))
            .bearer_auth(token)
            .json(req)
            .send()
            .await
            .map_err(|e| ApiError::Other(format!("patch failed: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }
        Ok(())
    }

    pub async fn delete_event(&mut self, calendar_id: &str, event_id: &str) -> Result<(), ApiError> {
        self.preflight_write().await?;
        let token = self.access_token().await?;
        let resp = self
            .http
            .delete(format!(
                "{}/calendar/v3/calendars/{}/events/{}",
                self.base_url,
                urlencoding::encode(calendar_id),
                urlencoding::encode(event_id)
            ))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| ApiError::Other(format!("delete failed: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(map_http_error(status, &body));
        }
        Ok(())
    }
}

fn map_event(_calendar_id: &str, e: EventDto) -> Result<CalendarEvent, ApiError> {
    let start = if let Some(dt) = e.start.date_time {
        let parsed = DateTime::<FixedOffset>::parse_from_rfc3339(&dt)
            .map_err(|_| ApiError::Other("Invalid event start dateTime".to_string()))?;
        EventTime::DateTime(parsed)
    } else {
        let d = NaiveDate::parse_from_str(&e.start.date.unwrap_or_default(), "%Y-%m-%d")
            .map_err(|_| ApiError::Other("Invalid event start date".to_string()))?;
        EventTime::AllDay(d)
    };

    let end = if let Some(dt) = e.end.date_time {
        let parsed = DateTime::<FixedOffset>::parse_from_rfc3339(&dt)
            .map_err(|_| ApiError::Other("Invalid event end dateTime".to_string()))?;
        EventTime::DateTime(parsed)
    } else {
        let d = NaiveDate::parse_from_str(&e.end.date.unwrap_or_default(), "%Y-%m-%d")
            .map_err(|_| ApiError::Other("Invalid event end date".to_string()))?;
        EventTime::AllDay(d)
    };

    Ok(CalendarEvent {
        id: e.id,
        title: e.summary.unwrap_or_else(|| "(no title)".to_string()),
        start,
        end,
        location: e.location,
        description: e.description,
        transparency: e.transparency,
        attendees_count: e.attendees.unwrap_or_default().len(),
        meet_link: e.hangout_link,
        timezone: e.start.time_zone.or(e.end.time_zone),
    })
}

async fn authorize_interactive(creds: &CredentialsFile) -> Result<StoredToken, ApiError> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| ApiError::Other(e.to_string()))?;
    let addr = listener.local_addr().map_err(|e| ApiError::Other(e.to_string()))?;
    let redirect = format!("http://127.0.0.1:{}/callback", addr.port());

    let client = BasicClient::new(
        ClientId::new(creds.installed.client_id.clone()),
        Some(ClientSecret::new(creds.installed.client_secret.clone().unwrap_or_default())),
        AuthUrl::new(creds.installed.auth_uri.clone()).map_err(|e| ApiError::Other(e.to_string()))?,
        Some(TokenUrl::new(creds.installed.token_uri.clone()).map_err(|e| ApiError::Other(e.to_string()))?),
    )
    .set_redirect_uri(RedirectUrl::new(redirect).map_err(|e| ApiError::Other(e.to_string()))?);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, _) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(READ_SCOPE.to_string()))
        .add_scope(Scope::new(WRITE_SCOPE.to_string()))
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
        .set_pkce_challenge(pkce_challenge)
        .url();
    open::that(url.as_str()).map_err(|e| ApiError::Other(format!("open browser failed: {e}")))?;

    let (mut stream, _) = listener.accept().await.map_err(|e| ApiError::Other(e.to_string()))?;
    let mut buf = [0u8; 4096];
    let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
        .await
        .map_err(|e| ApiError::Other(e.to_string()))?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let line = req.lines().next().ok_or_else(|| ApiError::Other("invalid callback".to_string()))?;
    let path = line.split_whitespace().nth(1).ok_or_else(|| ApiError::Other("malformed callback".to_string()))?;
    let parsed = url::Url::parse(&format!("http://localhost{path}")).map_err(|e| ApiError::Other(e.to_string()))?;
    let code = parsed
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| ApiError::Other("missing auth code".to_string()))?;

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h2>gtui Calendar authorized. You can close this window.</h2></body></html>";
    let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes()).await;

    let token = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(async_http_client)
        .await
        .map_err(|e| ApiError::Other(format!("token exchange failed: {e}")))?;

    let stored = StoredToken {
        access_token: token.access_token().secret().to_string(),
        refresh_token: token
            .refresh_token()
            .ok_or_else(|| ApiError::Other("missing refresh token".to_string()))?
            .secret()
            .to_string(),
        expires_at: now_unix() + token.expires_in().unwrap_or(Duration::from_secs(3600)).as_secs() as i64,
    };

    let path = storage::calendar_token_path().map_err(ApiError::Other)?;
    storage::write_json(&path, &stored).map_err(ApiError::Other)?;
    Ok(stored)
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn client(server: &MockServer) -> CalendarClient {
        CalendarClient::with_base_url_and_token(server.base_url(), "tok".to_string())
    }

    #[tokio::test]
    async fn list_calendars_ok() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/calendar/v3/users/me/calendarList");
                then.status(200).json_body_obj(&serde_json::json!({
                    "items":[{"id":"primary","summary":"Personal","primary":true}]
                }));
            })
            .await;

        let mut c = client(&server);
        let out = c.list_calendars().await.expect("cal list");
        assert_eq!(out.len(), 1);
        assert!(out[0].primary);
    }

    #[tokio::test]
    async fn list_events_has_range_params() {
        let server = MockServer::start_async().await;
        let m = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/calendar/v3/calendars/primary/events")
                    .query_param("singleEvents", "true")
                    .query_param("orderBy", "startTime");
                then.status(200).json_body_obj(&serde_json::json!({
                    "items":[{"id":"e1","summary":"S","start":{"date":"2026-03-05"},"end":{"date":"2026-03-06"}}]
                }));
            })
            .await;

        let mut c = client(&server);
        let out = c
            .list_events("primary", Utc::now(), Utc::now() + chrono::Duration::days(14))
            .await
            .expect("events");
        m.assert_async().await;
        assert_eq!(out.len(), 1);
    }

    #[tokio::test]
    async fn create_edit_delete_ok() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/tokeninfo");
                then.status(200)
                    .json_body_obj(&serde_json::json!({"scope":"https://www.googleapis.com/auth/calendar.events"}));
            })
            .await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/calendar/v3/calendars/primary/events");
                then.status(200).json_body_obj(&serde_json::json!({}));
            })
            .await;
        server
            .mock_async(|when, then| {
                when.method("PATCH")
                    .path("/calendar/v3/calendars/primary/events/e1");
                then.status(200).json_body_obj(&serde_json::json!({}));
            })
            .await;
        server
            .mock_async(|when, then| {
                when.method(DELETE)
                    .path("/calendar/v3/calendars/primary/events/e1");
                then.status(204);
            })
            .await;

        let mut c = client(&server);
        let req = EventInsertRequest {
            summary: "A".to_string(),
            start: crate::calendar::api::EventWriteDateTime {
                date_time: Some(Utc::now().to_rfc3339()),
                date: None,
                time_zone: Some("+00:00".to_string()),
            },
            end: crate::calendar::api::EventWriteDateTime {
                date_time: Some((Utc::now() + chrono::Duration::hours(1)).to_rfc3339()),
                date: None,
                time_zone: Some("+00:00".to_string()),
            },
            location: None,
            description: None,
        };
        c.insert_event("primary", &req).await.expect("insert");
        c.patch_event("primary", "e1", &req).await.expect("patch");
        c.delete_event("primary", "e1").await.expect("delete");
    }

    #[test]
    fn error_mapping() {
        let e = map_http_error(
            reqwest::StatusCode::FORBIDDEN,
            r#"{"error":{"errors":[{"reason":"accessNotConfigured"}],"message":"bad"}}"#,
        );
        assert!(matches!(e, ApiError::ApiNotEnabled));
        let e = map_http_error(reqwest::StatusCode::UNAUTHORIZED, "");
        assert!(matches!(e, ApiError::AuthExpired));
        let e = map_http_error(reqwest::StatusCode::TOO_MANY_REQUESTS, "");
        assert!(matches!(e, ApiError::RateLimited));
        let e = map_http_error(reqwest::StatusCode::BAD_GATEWAY, "");
        assert!(matches!(e, ApiError::Transient(502)));
    }
}
