use crate::db::store;
use crate::state::AppState;

#[tauri::command]
pub fn mark_important(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    store::mark_status(&state.db_path, &id, "IMPORTANT").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mark_ignored(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    store::mark_status(&state.db_path, &id, "IGNORED").map_err(|e| e.to_string())
}
