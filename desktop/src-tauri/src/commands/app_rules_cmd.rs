use crate::db::models::AppRuleRow;
use crate::db::store;
use crate::state::AppState;

#[tauri::command]
pub fn list_app_rules(state: tauri::State<'_, AppState>) -> Result<Vec<AppRuleRow>, String> {
    store::list_app_rules(&state.db_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_app_rule(
    package_name: String,
    flag: String,
    enabled: bool,
    state: tauri::State<'_, AppState>,
) -> Result<AppRuleRow, String> {
    store::set_app_rule_flag(&state.db_path, &package_name, &flag, enabled)
        .map_err(|e| e.to_string())
}
