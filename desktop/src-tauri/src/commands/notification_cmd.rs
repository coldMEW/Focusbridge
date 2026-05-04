use crate::db::models::NotificationRow;
use crate::db::store;
use crate::state::AppState;

#[tauri::command]
pub fn list_notifications(
    limit: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<NotificationRow>, String> {
    store::list_notifications(&state.db_path, limit.unwrap_or(100)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mark_important(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    store::mark_status(&state.db_path, &id, "IMPORTANT").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mark_ignored(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    store::mark_status(&state.db_path, &id, "IGNORED").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_notification(id: String, state: tauri::State<'_, AppState>) -> Result<usize, String> {
    store::delete_notification(&state.db_path, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_notifications_older_than(
    cutoff_ms: i64,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    store::clear_notifications_older_than(&state.db_path, cutoff_ms).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_notifications_between(
    start_ms: i64,
    end_ms: i64,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    store::clear_notifications_between(&state.db_path, start_ms, end_ms).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_all_notifications(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    store::clear_all_notifications(&state.db_path).map_err(|e| e.to_string())
}
