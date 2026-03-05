use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    RefreshToken, Scope, TokenResponse, TokenUrl,
};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

use crate::api_error::{map_http_error, ApiError};
use crate::gmail::api::{Header, LabelDto, LabelsResponse, MessagePart, MessageResponse, ThreadResponse, ThreadsListResponse};
use crate::gmail::models::{GmailLabel, MessageDetail, ThreadHeader};
use crate::storage;

#[derive(Debug, thiserror::Error)]
pub enum GmailError {
    #[error("{0}")]
    Api(#[from] ApiError),
    #[error("{0}")]
    Msg(String),
}

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

pub struct GmailClient {
    http: Client,
    oauth_client: BasicClient,
    token: StoredToken,
}

impl GmailClient {
    pub async fn from_disk_or_authorize() -> Result<Self, GmailError> {
        storage::ensure_config_dir().map_err(GmailError::Msg)?;
        storage::ensure_data_dir().map_err(GmailError::Msg)?;

        let creds_path = storage::credentials_path().map_err(GmailError::Msg)?;
        let creds: CredentialsFile = storage::read_json(&creds_path).map_err(GmailError::Msg)?;

        let auth_url = AuthUrl::new(creds.installed.auth_uri.clone()).map_err(|e| GmailError::Msg(e.to_string()))?;
        let token_url = TokenUrl::new(creds.installed.token_uri.clone()).map_err(|e| GmailError::Msg(e.to_string()))?;

        let oauth_client = BasicClient::new(
            ClientId::new(creds.installed.client_id.clone()),
            Some(ClientSecret::new(creds.installed.client_secret.clone().unwrap_or_default())),
            auth_url,
            Some(token_url),
        );

        let token_path = storage::gmail_token_path().map_err(GmailError::Msg)?;
        let token = if token_path.exists() {
            storage::read_json(&token_path).map_err(GmailError::Msg)?
        } else {
            authorize_interactive(&creds).await?
        };

        init_cache().map_err(GmailError::Msg)?;

        Ok(Self {
            http: Client::new(),
            oauth_client,
            token,
        })
    }

    async fn access_token(&mut self) -> Result<String, GmailError> {
        if self.token.expires_at - 60 > now_unix() {
            return Ok(self.token.access_token.clone());
        }

        let token_res = self
            .oauth_client
            .exchange_refresh_token(&RefreshToken::new(self.token.refresh_token.clone()))
            .request_async(async_http_client)
            .await
            .map_err(|e| GmailError::Msg(format!("Refresh failed: {e}")))?;

        self.token.access_token = token_res.access_token().secret().to_string();
        self.token.expires_at = now_unix() + token_res.expires_in().unwrap_or(Duration::from_secs(3600)).as_secs() as i64;
        if let Some(rt) = token_res.refresh_token() {
            self.token.refresh_token = rt.secret().to_string();
        }

        let token_path = storage::gmail_token_path().map_err(GmailError::Msg)?;
        storage::write_json(&token_path, &self.token).map_err(GmailError::Msg)?;
        Ok(self.token.access_token.clone())
    }

    pub async fn list_labels(&mut self) -> Result<Vec<GmailLabel>, GmailError> {
        let token = self.access_token().await?;
        let resp = self
            .http
            .get("https://gmail.googleapis.com/gmail/v1/users/me/labels")
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| GmailError::Msg(format!("labels request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(GmailError::Api(map_http_error(status, &body)));
        }

        let dto: LabelsResponse = resp.json().await.map_err(|e| GmailError::Msg(format!("labels decode failed: {e}")))?;
        Ok(dto
            .labels
            .unwrap_or_default()
            .into_iter()
            .map(|l: LabelDto| GmailLabel {
                id: l.id,
                name: l.name,
                label_type: l.label_type,
            })
            .collect())
    }

    pub async fn list_threads(
        &mut self,
        label: &str,
        query: Option<&str>,
    ) -> Result<Vec<ThreadHeader>, GmailError> {
        let token = self.access_token().await?;
        let mut req = self
            .http
            .get("https://gmail.googleapis.com/gmail/v1/users/me/threads")
            .query(&[("labelIds", label), ("maxResults", "30")]);
        if let Some(q) = query.filter(|q| !q.is_empty()) {
            req = req.query(&[("q", q)]);
        }

        let resp = req
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| GmailError::Msg(format!("threads request failed: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(GmailError::Api(map_http_error(status, &body)));
        }

        let dto: ThreadsListResponse = resp.json().await.map_err(|e| GmailError::Msg(format!("threads decode failed: {e}")))?;
        let mut out = Vec::new();
        for t in dto.threads.unwrap_or_default().into_iter().take(30) {
            let thread = self.get_thread_overview(&t.id).await?;
            out.push(thread);
        }

        cache_threads(label, &out).map_err(GmailError::Msg)?;
        Ok(out)
    }

    async fn get_thread_overview(&mut self, thread_id: &str) -> Result<ThreadHeader, GmailError> {
        let token = self.access_token().await?;
        let resp = self
            .http
            .get(format!("https://gmail.googleapis.com/gmail/v1/users/me/threads/{thread_id}"))
            .query(&[("format", "metadata")])
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| GmailError::Msg(format!("thread overview failed: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(GmailError::Api(map_http_error(status, &body)));
        }

        let dto: ThreadResponse = resp.json().await.map_err(|e| GmailError::Msg(format!("thread decode failed: {e}")))?;
        let messages = dto.messages.unwrap_or_default();
        let last = messages.last().ok_or_else(|| GmailError::Msg("thread had no messages".to_string()))?;
        let from = header_value(last, "From").unwrap_or_else(|| "(unknown)".to_string());
        let subject = header_value(last, "Subject").unwrap_or_else(|| "(no subject)".to_string());
        let date = header_value(last, "Date").unwrap_or_default();
        let unread = last
            .label_ids
            .as_ref()
            .map(|ls| ls.iter().any(|l| l == "UNREAD"))
            .unwrap_or(false);

        Ok(ThreadHeader {
            id: thread_id.to_string(),
            last_message_id: Some(last.id.clone()),
            from,
            subject,
            date,
            unread,
            snippet: last.snippet.clone().unwrap_or_default(),
        })
    }

    pub async fn get_message_detail(&mut self, message_id: &str) -> Result<MessageDetail, GmailError> {
        let token = self.access_token().await?;
        let resp = self
            .http
            .get(format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}"))
            .query(&[("format", "full")])
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| GmailError::Msg(format!("message request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(GmailError::Api(map_http_error(status, &body)));
        }

        let m: MessageResponse = resp.json().await.map_err(|e| GmailError::Msg(format!("message decode failed: {e}")))?;
        let from = header_value(&m, "From").unwrap_or_default();
        let to = header_value(&m, "To").unwrap_or_default();
        let subject = header_value(&m, "Subject").unwrap_or_default();
        let date = header_value(&m, "Date").unwrap_or_default();
        let body = extract_body(m.payload.as_ref()).unwrap_or_default();
        let body = strip_html(&body);

        Ok(MessageDetail {
            id: m.id,
            thread_id: m.thread_id.unwrap_or_default(),
            from,
            to,
            subject,
            date,
            snippet: m.snippet.unwrap_or_default(),
            labels: m.label_ids.unwrap_or_default(),
            body,
        })
    }

    pub async fn modify_thread_labels(
        &mut self,
        thread_id: &str,
        add_label_ids: &[&str],
        remove_label_ids: &[&str],
    ) -> Result<(), GmailError> {
        let token = self.access_token().await?;
        let payload = serde_json::json!({
            "addLabelIds": add_label_ids,
            "removeLabelIds": remove_label_ids,
        });

        let resp = self
            .http
            .post(format!("https://gmail.googleapis.com/gmail/v1/users/me/threads/{thread_id}/modify"))
            .bearer_auth(token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| GmailError::Msg(format!("modify failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(GmailError::Api(map_http_error(status, &body)));
        }
        Ok(())
    }

    pub async fn send_raw_message(&mut self, raw_message: String) -> Result<(), GmailError> {
        let token = self.access_token().await?;
        let raw = URL_SAFE_NO_PAD.encode(raw_message.as_bytes());
        let payload = serde_json::json!({ "raw": raw });

        let resp = self
            .http
            .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
            .bearer_auth(token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| GmailError::Msg(format!("send failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(GmailError::Api(map_http_error(status, &body)));
        }
        Ok(())
    }

    pub fn load_cached_threads(&self, label: &str) -> Result<Vec<ThreadHeader>, GmailError> {
        let cache_path = storage::cache_db_path().map_err(GmailError::Msg)?;
        let conn = Connection::open(cache_path).map_err(|e| GmailError::Msg(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT thread_id, from_header, subject, date_header, unread, snippet, last_message_id FROM thread_cache WHERE label = ? ORDER BY rowid DESC LIMIT 50")
            .map_err(|e| GmailError::Msg(e.to_string()))?;

        let mut rows = stmt.query([label]).map_err(|e| GmailError::Msg(e.to_string()))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(|e| GmailError::Msg(e.to_string()))? {
            out.push(ThreadHeader {
                id: row.get(0).map_err(|e| GmailError::Msg(e.to_string()))?,
                from: row.get(1).map_err(|e| GmailError::Msg(e.to_string()))?,
                subject: row.get(2).map_err(|e| GmailError::Msg(e.to_string()))?,
                date: row.get(3).map_err(|e| GmailError::Msg(e.to_string()))?,
                unread: row.get::<_, i64>(4).map_err(|e| GmailError::Msg(e.to_string()))? != 0,
                snippet: row.get(5).map_err(|e| GmailError::Msg(e.to_string()))?,
                last_message_id: row.get(6).ok(),
            });
        }
        Ok(out)
    }
}

fn extract_body(part: Option<&MessagePart>) -> Option<String> {
    let part = part?;
    if let Some(mt) = &part.mime_type {
        if mt == "text/plain" {
            if let Some(data) = part.body.as_ref().and_then(|b| b.data.as_ref()) {
                return decode_gmail_base64(data);
            }
        }
    }

    if let Some(parts) = &part.parts {
        for p in parts {
            if let Some(text) = extract_body(Some(p)) {
                return Some(text);
            }
        }
    }

    if let Some(data) = part.body.as_ref().and_then(|b| b.data.as_ref()) {
        return decode_gmail_base64(data);
    }
    None
}

fn decode_gmail_base64(input: &str) -> Option<String> {
    URL_SAFE_NO_PAD
        .decode(input.as_bytes())
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

fn strip_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    let mut just_added_space = false;
    for c in input.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                if !just_added_space {
                    out.push(' ');
                    just_added_space = true;
                }
            }
            _ if !in_tag => {
                if c.is_whitespace() {
                    if !just_added_space {
                        out.push(' ');
                        just_added_space = true;
                    }
                } else {
                    out.push(c);
                    just_added_space = false;
                }
            }
            _ => {}
        }
    }
    out = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    out.trim().to_string()
}

fn header_value(m: &MessageResponse, name: &str) -> Option<String> {
    m.payload
        .as_ref()
        .and_then(|p| p.headers.as_ref())
        .and_then(|hs| find_header(hs, name))
}

fn find_header(headers: &[Header], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .map(|h| h.value.clone())
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs() as i64
}

async fn authorize_interactive(creds: &CredentialsFile) -> Result<StoredToken, GmailError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| GmailError::Msg(format!("bind callback failed: {e}")))?;
    let addr = listener.local_addr().map_err(|e| GmailError::Msg(e.to_string()))?;
    let redirect = format!("http://127.0.0.1:{}/callback", addr.port());

    let client = BasicClient::new(
        ClientId::new(creds.installed.client_id.clone()),
        Some(ClientSecret::new(creds.installed.client_secret.clone().unwrap_or_default())),
        AuthUrl::new(creds.installed.auth_uri.clone()).map_err(|e| GmailError::Msg(e.to_string()))?,
        Some(TokenUrl::new(creds.installed.token_uri.clone()).map_err(|e| GmailError::Msg(e.to_string()))?),
    )
    .set_redirect_uri(RedirectUrl::new(redirect).map_err(|e| GmailError::Msg(e.to_string()))?);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, _csrf) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("https://www.googleapis.com/auth/gmail.readonly".to_string()))
        .add_scope(Scope::new("https://www.googleapis.com/auth/gmail.modify".to_string()))
        .add_scope(Scope::new("https://www.googleapis.com/auth/gmail.send".to_string()))
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
        .set_pkce_challenge(pkce_challenge)
        .url();

    open::that(authorize_url.as_str()).map_err(|e| GmailError::Msg(format!("open browser failed: {e}")))?;

    let (mut stream, _) = listener.accept().await.map_err(|e| GmailError::Msg(e.to_string()))?;
    let mut buf = [0u8; 4096];
    let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
        .await
        .map_err(|e| GmailError::Msg(e.to_string()))?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let line = req.lines().next().ok_or_else(|| GmailError::Msg("invalid callback".to_string()))?;
    let path = line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| GmailError::Msg("malformed callback".to_string()))?;
    let parsed = url::Url::parse(&format!("http://localhost{path}")).map_err(|e| GmailError::Msg(e.to_string()))?;
    let code = parsed
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| GmailError::Msg("missing auth code".to_string()))?;

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h2>gtui Gmail authorized. You can close this window.</h2></body></html>";
    let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes()).await;

    let token = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(async_http_client)
        .await
        .map_err(|e| GmailError::Msg(format!("token exchange failed: {e}")))?;

    let stored = StoredToken {
        access_token: token.access_token().secret().to_string(),
        refresh_token: token
            .refresh_token()
            .ok_or_else(|| GmailError::Msg("missing refresh token".to_string()))?
            .secret()
            .to_string(),
        expires_at: now_unix() + token.expires_in().unwrap_or(Duration::from_secs(3600)).as_secs() as i64,
    };

    let path = storage::gmail_token_path().map_err(GmailError::Msg)?;
    storage::write_json(&path, &stored).map_err(GmailError::Msg)?;
    Ok(stored)
}

fn init_cache() -> Result<(), String> {
    let cache_path = storage::cache_db_path()?;
    let conn = Connection::open(cache_path).map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS thread_cache (
            label TEXT NOT NULL,
            thread_id TEXT NOT NULL,
            from_header TEXT NOT NULL,
            subject TEXT NOT NULL,
            date_header TEXT NOT NULL,
            unread INTEGER NOT NULL,
            snippet TEXT NOT NULL,
            last_message_id TEXT,
            PRIMARY KEY(label, thread_id)
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn cache_threads(label: &str, threads: &[ThreadHeader]) -> Result<(), String> {
    let cache_path = storage::cache_db_path()?;
    let mut conn = Connection::open(cache_path).map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM thread_cache WHERE label = ?", [label])
        .map_err(|e| e.to_string())?;

    for t in threads {
        tx.execute(
            "INSERT OR REPLACE INTO thread_cache (label, thread_id, from_header, subject, date_header, unread, snippet, last_message_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                label,
                t.id,
                t.from,
                t.subject,
                t.date,
                if t.unread { 1 } else { 0 },
                t.snippet,
                t.last_message_id,
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
