use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    RefreshToken, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

use crate::storage;

#[derive(Debug, Deserialize)]
pub struct CredentialsFile {
    pub installed: InstalledCredentials,
}

#[derive(Debug, Deserialize)]
pub struct InstalledCredentials {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub auth_uri: String,
    pub token_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

pub struct AuthManager {
    oauth_client: BasicClient,
    token: StoredToken,
}

impl AuthManager {
    pub async fn from_disk_or_authorize() -> Result<Self, String> {
        storage::ensure_config_dir()?;
        let creds_path = storage::credentials_path()?;
        let creds: CredentialsFile = storage::read_json(&creds_path)?;

        let auth_url = AuthUrl::new(creds.installed.auth_uri.clone())
            .map_err(|e| format!("Invalid auth_uri: {e}"))?;
        let token_url = TokenUrl::new(creds.installed.token_uri.clone())
            .map_err(|e| format!("Invalid token_uri: {e}"))?;

        let mut token: Option<StoredToken> = None;
        let token_path = storage::token_path()?;
        if token_path.exists() {
            let t: StoredToken = storage::read_json(&token_path)?;
            token = Some(t);
        }

        if token.is_none() {
            token = Some(authorize_interactive(&creds).await?);
        }

        let client_secret = creds
            .installed
            .client_secret
            .clone()
            .unwrap_or_default();

        let oauth_client = BasicClient::new(
            ClientId::new(creds.installed.client_id.clone()),
            Some(ClientSecret::new(client_secret)),
            auth_url,
            Some(token_url),
        );

        Ok(Self {
            oauth_client,
            token: token.expect("token set"),
        })
    }

    pub async fn access_token(&mut self) -> Result<String, String> {
        let now = now_unix();
        if self.token.expires_at - 60 > now {
            return Ok(self.token.access_token.clone());
        }

        let token_res = self
            .oauth_client
            .exchange_refresh_token(&RefreshToken::new(self.token.refresh_token.clone()))
            .request_async(async_http_client)
            .await
            .map_err(|e| format!("Refresh token exchange failed: {e}"))?;

        self.token.access_token = token_res.access_token().secret().to_string();
        self.token.expires_at = now
            + token_res
                .expires_in()
                .unwrap_or(Duration::from_secs(3600))
                .as_secs() as i64;
        if let Some(rt) = token_res.refresh_token() {
            self.token.refresh_token = rt.secret().to_string();
        }

        let token_path = storage::token_path()?;
        storage::write_json(&token_path, &self.token)?;
        Ok(self.token.access_token.clone())
    }
}

async fn authorize_interactive(creds: &CredentialsFile) -> Result<StoredToken, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("Failed binding localhost callback: {e}"))?;
    let addr = listener
        .local_addr()
        .map_err(|e| format!("Failed getting callback port: {e}"))?;

    let redirect = format!("http://127.0.0.1:{}/callback", addr.port());

    let auth_url = AuthUrl::new(creds.installed.auth_uri.clone())
        .map_err(|e| format!("Invalid auth_uri: {e}"))?;
    let token_url = TokenUrl::new(creds.installed.token_uri.clone())
        .map_err(|e| format!("Invalid token_uri: {e}"))?;

    let client = BasicClient::new(
        ClientId::new(creds.installed.client_id.clone()),
        Some(ClientSecret::new(
            creds.installed.client_secret.clone().unwrap_or_default(),
        )),
        auth_url,
        Some(token_url),
    )
    .set_redirect_uri(
        RedirectUrl::new(redirect.clone()).map_err(|e| format!("Invalid redirect URI: {e}"))?,
    );

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, _csrf) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/tasks".to_string(),
        ))
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
        .set_pkce_challenge(pkce_challenge)
        .url();

    open::that(authorize_url.as_str())
        .map_err(|e| format!("Failed to open browser automatically: {e}"))?;

    let code = receive_code(listener, addr).await?;

    let token = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(async_http_client)
        .await
        .map_err(|e| format!("Authorization code exchange failed: {e}"))?;

    let refresh = token
        .refresh_token()
        .ok_or_else(|| "Google did not return a refresh token".to_string())?
        .secret()
        .to_string();

    let stored = StoredToken {
        access_token: token.access_token().secret().to_string(),
        refresh_token: refresh,
        expires_at: now_unix()
            + token
                .expires_in()
                .unwrap_or(Duration::from_secs(3600))
                .as_secs() as i64,
    };

    let token_path = storage::token_path()?;
    storage::write_json(&token_path, &stored)?;
    Ok(stored)
}

async fn receive_code(listener: TcpListener, _addr: SocketAddr) -> Result<String, String> {
    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|e| format!("Failed receiving OAuth callback: {e}"))?;

    let mut buf = [0u8; 4096];
    let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
        .await
        .map_err(|e| format!("Failed reading callback request: {e}"))?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let first_line = req
        .lines()
        .next()
        .ok_or_else(|| "Invalid callback HTTP request".to_string())?;
    let path = first_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "Malformed callback request line".to_string())?;

    let url = format!("http://localhost{path}");
    let parsed = url::Url::parse(&url).map_err(|e| format!("Malformed callback URL: {e}"))?;

    let code = parsed
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| "No authorization code in callback".to_string())?;

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h2>gtui authorized. You can close this window.</h2></body></html>";
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.flush().await;

    Ok(code)
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs() as i64
}
