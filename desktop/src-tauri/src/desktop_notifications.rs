use crate::db::models::NotificationRow;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;
use tracing::warn;

pub fn show_phone_notification<R: Runtime>(app: &AppHandle<R>, row: &NotificationRow) {
    if !should_notify(app) {
        return;
    }

    let title = if row.sender.trim().is_empty() {
        row.app_name.clone()
    } else {
        format!("{} - {}", row.app_name, row.sender)
    };
    let body = if row.content_hidden != 0 {
        "Masked message - open FocusBridge to peek".to_string()
    } else if row.message.trim().is_empty() {
        "New notification".to_string()
    } else {
        row.message.clone()
    };

    if let Err(error) = app.notification().builder().title(title).body(body).show() {
        warn!(%error, "failed to show desktop notification");
    }
}

fn should_notify<R: Runtime>(app: &AppHandle<R>) -> bool {
    let Some(window) = app.get_webview_window("main") else {
        return true;
    };

    let visible = window.is_visible().unwrap_or(true);
    let minimized = window.is_minimized().unwrap_or(false);
    let focused = window.is_focused().unwrap_or(false);

    !visible || minimized || !focused
}
