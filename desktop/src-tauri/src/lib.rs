pub mod commands;
pub mod db;
pub mod desktop_notifications;
pub mod pairing;
pub mod priority;
pub mod server;
pub mod state;
pub mod sync;
pub mod tray;

use crate::state::AppState;
use anyhow::Context;
use tauri::{Emitter, Manager, WindowEvent};
use tracing::info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:focusbridge.db", db::migrations::list())
                .build(),
        )
        .setup(|app| {
            let handle = app.handle().clone();
            let app_data_dir = app
                .path()
                .app_data_dir()
                .context("resolve app data directory")?;
            let db_path = app_data_dir.join("focusbridge.db");
            db::store::init(&db_path)?;
            let app_state = AppState::new(db_path);
            app.manage(app_state.clone());
            info!("focusbridge-desktop setup");
            tray::menu::install(&handle)?;
            tauri::async_runtime::spawn(server::ws_server::start(
                server::ws_server::WsServerConfig {
                    bind: "0.0.0.0:9173".parse().expect("valid websocket bind"),
                },
                app_state,
                handle,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pairing_cmd::generate_pairing_qr,
            commands::pairing_cmd::consume_pairing,
            commands::settings_cmd::get_settings,
            commands::settings_cmd::set_study_mode,
            commands::notification_cmd::list_notifications,
            commands::notification_cmd::mark_important,
            commands::notification_cmd::mark_ignored,
            minimize_to_tray,
            quit_app,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.emit("focusbridge://close-requested", ());
                let _ = window.show();
                let _ = window.set_focus();
            }
        })
        .run(tauri::generate_context!())
        .expect("tauri runtime error");
}

#[tauri::command]
fn minimize_to_tray(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}
