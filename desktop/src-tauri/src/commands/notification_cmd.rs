use crate::db::models::NotificationRow;
use crate::db::store;
use crate::state::AppState;

/// The inbox, once the local vault is open.
///
/// The interface only asks for this after unlocking, so refusing while locked
/// changes nothing in normal use. It matters for the case the lock exists for: a
/// second way to read the messages should not be sitting there answering anyone
/// who asks, whether that is a bug in the interface or something injected into
/// it. The lock is a property of the data, not of which screen is drawn.
#[tauri::command]
pub fn list_notifications(
    limit: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<NotificationRow>, String> {
    inbox_is_readable(&state)?;
    store::list_notifications(&state.db_path, limit.unwrap_or(100)).map_err(|e| e.to_string())
}

/// The one place that decides whether stored messages may be handed out.
pub(crate) fn inbox_is_readable(state: &AppState) -> Result<(), String> {
    if state.vault_is_unlocked() {
        Ok(())
    } else {
        Err("unlock FocusBridge to read notifications".into())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn locked_state() -> AppState {
        let dir = std::env::temp_dir().join(format!("fb-inbox-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cert = focusbridge_core::cert::generate_self_signed("focusbridge-test")
            .expect("generate a certificate for the test");
        AppState::new(dir.join("test.db"), cert)
    }

    #[test]
    fn the_inbox_is_refused_while_locked_and_served_once_open() {
        // The lock belongs to the data, not to whichever screen is drawn: a
        // second way to read the messages must not sit there answering anyone
        // who asks, whether that is a bug in the interface or something injected
        // into it.
        let state = locked_state();
        assert!(inbox_is_readable(&state).is_err());
        state.unlock_vault();
        assert!(inbox_is_readable(&state).is_ok());
    }
}
