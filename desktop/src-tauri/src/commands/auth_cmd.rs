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
    pub lock_timeout_minutes: u32,
    pub recovery_configured: bool,
    pub recovery_question: Option<String>,
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
        lock_timeout_minutes: store::get_setting(&state.db_path, "lock_timeout_minutes")
            .ok()
            .flatten()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
        recovery_configured: store::get_setting(&state.db_path, "auth_recovery_answer_hash")
            .ok()
            .flatten()
            .is_some(),
        recovery_question: store::get_setting(&state.db_path, "auth_recovery_question")
            .ok()
            .flatten(),
    }
}

#[tauri::command]
pub fn auth_register(password: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    validate_password(&password)?;
    set_password(&state, &password)
}

#[tauri::command]
pub fn auth_register_with_recovery(
    password: String,
    security_question: String,
    security_answer: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    validate_password(&password)?;
    validate_recovery(&security_question, &security_answer)?;
    set_password(&state, &password)?;
    set_recovery(&state, &security_question, &security_answer)
}

#[tauri::command]
pub fn auth_recovery_question(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    store::get_setting(&state.db_path, "auth_recovery_question").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn auth_reset_password_with_recovery(
    security_answer: String,
    new_password: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    validate_password(&new_password)?;
    verify_recovery_answer(&state, &security_answer)?;
    set_password(&state, &new_password)
}

fn set_password(state: &AppState, password: &str) -> Result<(), String> {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    let hash = hash_password(password, &salt);
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

fn validate_recovery(question: &str, answer: &str) -> Result<(), String> {
    if question.trim().len() < 8 {
        return Err("security question must be at least 8 characters".into());
    }
    if normalize_security_answer(answer).len() < 2 {
        return Err("security answer must be at least 2 characters".into());
    }
    Ok(())
}

fn set_recovery(state: &AppState, question: &str, answer: &str) -> Result<(), String> {
    let normalized = normalize_security_answer(answer);
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    let hash = hash_password(&normalized, &salt);
    store::set_setting(&state.db_path, "auth_recovery_question", question.trim())
        .map_err(|e| e.to_string())?;
    store::set_setting(
        &state.db_path,
        "auth_recovery_answer_salt",
        &hex::encode(salt),
    )
    .map_err(|e| e.to_string())?;
    store::set_setting(
        &state.db_path,
        "auth_recovery_answer_hash",
        &hex::encode(hash),
    )
    .map_err(|e| e.to_string())
}

fn verify_recovery_answer(state: &AppState, answer: &str) -> Result<(), String> {
    let salt = store::get_setting(&state.db_path, "auth_recovery_answer_salt")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "security recovery is not configured".to_string())?;
    let expected = store::get_setting(&state.db_path, "auth_recovery_answer_hash")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "security recovery is not configured".to_string())?;
    let salt = hex::decode(salt).map_err(|_| "stored recovery salt is invalid".to_string())?;
    let normalized = normalize_security_answer(answer);
    let actual = hash_password(&normalized, &salt);
    if constant_time_eq(&hex::encode(actual), &expected) {
        Ok(())
    } else {
        Err("security answer is incorrect".into())
    }
}

fn normalize_security_answer(answer: &str) -> String {
    answer
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleSignInResult {
    pub email: String,
    pub user_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaySignInResult {
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
pub async fn auth_relay_otp_start(
    relay_url: String,
    email: String,
    password: String,
) -> Result<(), String> {
    let relay_url = normalize_relay_url(&relay_url)?;
    let response = reqwest::Client::new()
        .post(format!("{relay_url}/auth/otp/start"))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .context("send otp start request")
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("read otp start response")
        .map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("otp start failed ({status}): {text}"));
    }
    Ok(())
}

#[tauri::command]
pub async fn auth_relay_otp_verify(
    relay_url: String,
    email: String,
    password: String,
    otp: String,
    state: tauri::State<'_, AppState>,
) -> Result<RelaySignInResult, String> {
    let relay_url = normalize_relay_url(&relay_url)?;
    let response = reqwest::Client::new()
        .post(format!("{relay_url}/auth/otp/verify"))
        .json(&serde_json::json!({ "email": email, "password": password, "otp": otp }))
        .send()
        .await
        .context("send otp verify request")
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("read otp verify response")
        .map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("otp verify failed ({status}): {text}"));
    }
    let relay: RelayAuthResponse = serde_json::from_str(&text)
        .context("decode relay otp auth response")
        .map_err(|e| e.to_string())?;
    store::set_setting(&state.db_path, "relay_url", &relay_url).map_err(|e| e.to_string())?;
    store::set_setting(&state.db_path, "relay_auth_token", &relay.token)
        .map_err(|e| e.to_string())?;
    store::set_setting(&state.db_path, "relay_auth_email", &relay.email)
        .map_err(|e| e.to_string())?;
    store::set_setting(&state.db_path, "relay_auth_user_id", &relay.user_id)
        .map_err(|e| e.to_string())?;
    Ok(RelaySignInResult {
        email: relay.email,
        user_id: relay.user_id,
    })
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
    if password.chars().all(|ch| ch.is_ascii_digit()) {
        if password.len() < 4 {
            return Err("PIN must be at least 4 digits".into());
        }
        return Ok(());
    }
    if password.len() < 8 {
        return Err("password must be at least 8 characters".into());
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
    let response_body =
        "FocusBridge received the Google authorization code. Return to the app to finish sign-in.";
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
    let response = reqwest::Client::new()
        .post(oauth_pkce::GOOGLE_TOKEN_URL)
        .form(&body)
        .send()
        .await
        .context("send google token request")
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("read google token response")
        .map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("google token exchange failed ({status}): {text}"));
    }
    serde_json::from_str::<GoogleTokenResponse>(&text)
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
