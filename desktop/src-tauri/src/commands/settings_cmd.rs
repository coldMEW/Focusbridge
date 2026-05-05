use crate::db::store;
use crate::state::AppState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettingsSnapshot {
    pub study_mode_enabled: bool,
    pub two_fa_mode_enabled: bool,
    pub blocked_apps: Vec<String>,
    pub priority_apps: Vec<String>,
    pub favorite_contacts: Vec<String>,
    pub priority_keywords: Vec<String>,
    pub blocked_keywords: Vec<String>,
    pub sync_mode: String,
    pub lock_timeout_minutes: u32,
}

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, AppState>) -> SettingsSnapshot {
    let study_mode_enabled = store::get_setting(&state.db_path, "study_mode_enabled")
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(false);
    SettingsSnapshot {
        study_mode_enabled,
        priority_keywords: csv_setting(&state, "priority_keywords"),
        blocked_keywords: csv_setting(&state, "blocked_keywords"),
        favorite_contacts: csv_setting(&state, "favorite_contacts"),
        lock_timeout_minutes: store::get_setting(&state.db_path, "lock_timeout_minutes")
            .ok()
            .flatten()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
        sync_mode: "LOCAL".into(),
        ..Default::default()
    }
}

#[tauri::command]
pub fn set_study_mode(on: bool, state: tauri::State<'_, AppState>) -> Result<(), String> {
    store::set_setting(
        &state.db_path,
        "study_mode_enabled",
        if on { "true" } else { "false" },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_rule_text(
    key: String,
    values: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    match key.as_str() {
        "priority_keywords" | "blocked_keywords" | "favorite_contacts" => {}
        _ => return Err(format!("unsupported rule text key: {key}")),
    }
    let normalized = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(",");
    store::set_setting(&state.db_path, &key, &normalized).map_err(|e| e.to_string())?;
    if let Ok(message) = store::rules_update_envelope(&state.db_path) {
        let _ = state.send_to_phone(message);
    }
    Ok(())
}

#[tauri::command]
pub fn set_lock_timeout_minutes(
    minutes: u32,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if minutes > 12 * 30 * 24 * 60 {
        return Err("lock timeout cannot exceed 12 months".into());
    }
    store::set_setting(&state.db_path, "lock_timeout_minutes", &minutes.to_string())
        .map_err(|e| e.to_string())
}

fn csv_setting(state: &tauri::State<'_, AppState>, key: &str) -> Vec<String> {
    store::get_setting(&state.db_path, key)
        .ok()
        .flatten()
        .unwrap_or_default()
        .split([',', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}
