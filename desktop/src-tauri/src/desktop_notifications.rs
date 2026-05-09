use crate::db::models::NotificationRow;
use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;
use tracing::warn;

pub fn show_phone_notification<R: Runtime>(app: &AppHandle<R>, row: &NotificationRow) {
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

pub fn show_connection_notification<R: Runtime>(app: &AppHandle<R>, connected: bool) {
    let (title, body) = if connected {
        (
            "FocusBridge connected",
            "Your phone is now syncing notifications.",
        )
    } else {
        (
            "FocusBridge disconnected",
            "Phone sync stopped. Reopen FocusBridge on Android or check Wi-Fi.",
        )
    };

    if let Err(error) = app.notification().builder().title(title).body(body).show() {
        warn!(%error, "failed to show connection notification");
    }
}
