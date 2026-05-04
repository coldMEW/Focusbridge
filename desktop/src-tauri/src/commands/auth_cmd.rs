use crate::db::store;
use crate::state::AppState;
use anyhow::{anyhow, Context};
use focusbridge_core::oauth_pkce;
use hmac::Hmac;
use pbkdf2::pbkdf2;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};

const ITERATIONS: u32 = 210_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub configured: bool,
    pub relay_email: Option<String>,
}

#[tauri::command]
pub fn auth_status(state: tauri::State<'_, AppState>) -> AuthStatus {
    AuthStatus {
        configured: store::get_setting(&state.db_path, "auth_password_hash")
            .ok()
            .flatten()
            .is_some(),
        relay_email: store::get_setting(&state.db_path, "relay_auth_email")
            .ok()
            .flatten(),
    }
}

#[tauri::command]
pub fn auth_register(password: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    validate_password(&password)?;
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    let hash = hash_password(&password, &salt);
    store::set_setting(&state.db_path, "auth_password_salt", &hex::encode(salt))
        .map_err(|e| e.to_string())?;
    store::set_setting(&state.db_path, "auth_password_hash", &hex::encode(hash))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn auth_login(password: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let salt = store::get_setting(&state.db_path, "auth_password_salt")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "password is not configured".to_string())?;
    let expected = store::get_setting(&state.db_path, "auth_password_hash")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "password is not configured".to_string())?;
    let salt = hex::decode(salt).map_err(|_| "stored salt is invalid".to_string())?;
    let actual = hash_password(&password, &salt);
    if constant_time_eq(&hex::encode(actual), &expected) {
        Ok(())
    } else {
        Err("invalid password".into())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleSignInResult {
    pub email: String,
    pub user_id: String,
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    id_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelayAuthResponse {
    user_id: String,
    email: String,
    token: String,
}

#[tauri::command]
pub async fn auth_google_sign_in(
    relay_url: String,
    state: tauri::State<'_, AppState>,
) -> Result<GoogleSignInResult, String> {
    let client_id = google_client_id().map_err(|e| e.to_string())?;
    let relay_url = normalize_relay_url(&relay_url)?;
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/oauth/google/callback");
    let verifier = oauth_pkce::code_verifier();
    let oauth_state = oauth_pkce::code_verifier();
    let authorize_url =
        oauth_pkce::google_authorize_url(&client_id, &redirect_uri, &oauth_state, &verifier);
    open::that(&authorize_url).map_err(|e| format!("open browser: {e}"))?;

    let code = wait_for_oauth_code(listener, &oauth_state).await?;
    let google = exchange_google_code(&client_id, &redirect_uri, &verifier, &code).await?;
    let relay = exchange_relay_google_token(&relay_url, &google.id_token).await?;

    store::set_setting(&state.db_path, "relay_url", &relay_url).map_err(|e| e.to_string())?;
    store::set_setting(&state.db_path, "relay_auth_token", &relay.token)
        .map_err(|e| e.to_string())?;
    store::set_setting(&state.db_path, "relay_auth_email", &relay.email)
        .map_err(|e| e.to_string())?;
    store::set_setting(&state.db_path, "relay_auth_user_id", &relay.user_id)
        .map_err(|e| e.to_string())?;

    Ok(GoogleSignInResult {
        email: relay.email,
        user_id: relay.user_id,
    })
}

fn validate_password(password: &str) -> Result<(), String> {
    if password.len() < 10 {
        return Err("password must be at least 10 characters".into());
    }
    Ok(())
}

fn hash_password(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    pbkdf2::<Hmac<Sha256>>(password.as_bytes(), salt, ITERATIONS, &mut out)
        .expect("pbkdf2 output length is fixed");
    out
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

fn google_client_id() -> anyhow::Result<String> {
    local_env_value("FOCUSBRIDGE_GOOGLE_CLIENT_ID")
        .or_else(|| std::env::var("FOCUSBRIDGE_GOOGLE_CLIENT_ID").ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("FOCUSBRIDGE_GOOGLE_CLIENT_ID is not configured"))
}

fn local_env_value(key: &str) -> Option<String> {
    for path in [
        Path::new(".env.local"),
        Path::new("../.env.local"),
        Path::new("../../.env.local"),
    ] {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((candidate, value)) = line.split_once('=') else {
                continue;
            };
            if candidate.trim() == key {
                return Some(value.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

fn normalize_relay_url(value: &str) -> Result<String, String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("relay URL is required".into());
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("https://{trimmed}"))
    }
}

async fn wait_for_oauth_code(
    listener: TcpListener,
    expected_state: &str,
) -> Result<String, String> {
    let accepted = timeout(Duration::from_secs(180), listener.accept())
        .await
        .map_err(|_| "Google sign-in timed out".to_string())?
        .map_err(|e| e.to_string())?;
    let (mut stream, _) = accepted;
    let mut buffer = vec![0u8; 8192];
    let size = stream.read(&mut buffer).await.map_err(|e| e.to_string())?;
    let request = String::from_utf8_lossy(&buffer[..size]);
    let response_body = "FocusBridge sign-in complete. You can close this tab.";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response_body.len(),
        response_body
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let (code, returned_state) = oauth_pkce::callback_code_and_state(&request)
        .ok_or_else(|| "Google callback was missing code/state".to_string())?;
    if returned_state != expected_state {
        return Err("Google sign-in state mismatch".into());
    }
    Ok(code)
}

async fn exchange_google_code(
    client_id: &str,
    redirect_uri: &str,
    verifier: &str,
    code: &str,
) -> Result<GoogleTokenResponse, String> {
    let mut body = HashMap::new();
    body.insert("client_id", client_id);
    body.insert("code", code);
    body.insert("code_verifier", verifier);
    body.insert("grant_type", "authorization_code");
    body.insert("redirect_uri", redirect_uri);
    reqwest::Client::new()
        .post(oauth_pkce::GOOGLE_TOKEN_URL)
        .form(&body)
        .send()
        .await
        .context("send google token request")
        .map_err(|e| e.to_string())?
        .error_for_status()
        .context("google token exchange failed")
        .map_err(|e| e.to_string())?
        .json::<GoogleTokenResponse>()
        .await
        .context("decode google token response")
        .map_err(|e| e.to_string())
}

async fn exchange_relay_google_token(
    relay_url: &str,
    id_token: &str,
) -> Result<RelayAuthResponse, String> {
    reqwest::Client::new()
        .post(format!("{relay_url}/auth/google"))
        .json(&serde_json::json!({ "id_token": id_token }))
        .send()
        .await
        .context("send relay google auth request")
        .map_err(|e| e.to_string())?
        .error_for_status()
        .context("relay google auth failed")
        .map_err(|e| e.to_string())?
        .json::<RelayAuthResponse>()
        .await
        .context("decode relay auth response")
        .map_err(|e| e.to_string())
}
