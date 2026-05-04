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
}

#[tauri::command]
pub fn auth_status(state: tauri::State<'_, AppState>) -> AuthStatus {
    AuthStatus {
        configured: store::get_setting(&state.db_path, "auth_password_hash")
            .ok()
            .flatten()
            .is_some(),
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
