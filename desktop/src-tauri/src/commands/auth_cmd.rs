use crate::db::store;
use crate::state::AppState;
use hmac::Hmac;
use pbkdf2::pbkdf2;
use rand::RngCore;
use serde::Serialize;
use sha2::Sha256;

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
    set_password(&state, &password)?;
    // Creating the vault leaves it open; the user is standing right there.
    state.unlock_vault();
    Ok(())
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
    set_recovery(&state, &security_question, &security_answer)?;
    state.unlock_vault();
    Ok(())
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

#[tauri::command]
pub fn auth_update_recovery(
    security_question: String,
    security_answer: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    validate_recovery(&security_question, &security_answer)?;
    set_recovery(&state, &security_question, &security_answer)
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
        state.unlock_vault();
        Ok(())
    } else {
        Err("invalid password".into())
    }
}

/// Locks the vault again: the idle timeout, or signing out.
///
/// The interface hiding itself is not enough on its own, because the backend
/// raises desktop notifications whether or not anything is on screen.
#[tauri::command]
pub fn auth_lock(state: tauri::State<'_, AppState>) {
    state.lock_vault();
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
