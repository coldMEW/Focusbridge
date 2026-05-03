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
    pub sync_mode: String,
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
