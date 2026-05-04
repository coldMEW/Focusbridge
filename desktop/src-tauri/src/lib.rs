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
    configure_platform_identity();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let app_data_dir = app
                .path()
                .app_data_dir()
                .context("resolve app data directory")?;
            let db_path = app_data_dir.join("focusbridge.db");
            let cert = pairing::cert_manager::load_or_generate(&app_data_dir)?;
            db::store::init(&db_path)?;
            let app_state = AppState::new(db_path, cert);
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
            commands::auth_cmd::auth_status,
            commands::auth_cmd::auth_register,
            commands::auth_cmd::auth_login,
            commands::auth_cmd::auth_relay_otp_start,
            commands::auth_cmd::auth_relay_otp_verify,
            commands::auth_cmd::auth_google_sign_in,
            commands::pairing_cmd::generate_pairing_qr,
            commands::pairing_cmd::consume_pairing,
            commands::settings_cmd::get_settings,
            commands::settings_cmd::set_study_mode,
            commands::settings_cmd::set_rule_text,
            commands::notification_cmd::list_notifications,
            commands::notification_cmd::mark_important,
            commands::notification_cmd::mark_ignored,
            commands::notification_cmd::delete_notification,
            commands::notification_cmd::clear_notifications_older_than,
            commands::notification_cmd::clear_notifications_between,
            commands::notification_cmd::clear_all_notifications,
            commands::app_rules_cmd::list_app_rules,
            commands::app_rules_cmd::set_app_rule,
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

#[cfg(windows)]
fn configure_platform_identity() {
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    let app_id: Vec<u16> = "com.focusbridge.desktop\0".encode_utf16().collect();
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
    }
}

#[cfg(not(windows))]
fn configure_platform_identity() {}

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
