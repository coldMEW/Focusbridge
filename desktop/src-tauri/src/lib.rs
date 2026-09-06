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

/// The LAN listener's port. The relay bridge dials it over loopback so both
/// transports converge on one tested application path.
const LOCAL_WS_PORT: u16 = 9173;
use tauri::{Emitter, Manager, WindowEvent};
use tracing::info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Chooses the TLS implementation before anything opens a connection.
///
/// From rustls 0.23 a process must say which provider it wants when more than one
/// could be linked, and asking later panics on whichever worker thread got there
/// first. Every test passed without this: the panic only appears once a real TLS
/// connection is made, which is to say once the app is actually used. So it is
/// installed here, at the top of startup, before the listener or the relay client
/// exists.
fn install_crypto_provider() {
    // An error means a provider is already installed, which is equally fine.
    let _ = rustls::crypto::ring::default_provider().install_default();
}

pub fn run() {
    configure_platform_identity();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();

    install_crypto_provider();

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
            let database = db::encrypted::initialize(&db_path)?;
            db::store::init(&db_path)?;
            app.manage(database);
            let cert = pairing::cert_manager::load_or_generate(&app_data_dir)?;
            let app_state = AppState::new(db_path, cert);
            app.manage(app_state.clone());
            info!("focusbridge-desktop setup");
            tray::menu::install(&handle)?;
            tauri::async_runtime::spawn(server::ws_server::start(
                server::ws_server::WsServerConfig {
                    bind: "0.0.0.0:9173".parse().expect("valid websocket bind"),
                },
                app_state.clone(),
                handle,
            ));
            // Cross-network sync. This is idle until the user enables the relay,
            // and it never affects the LAN listener above.
            tauri::async_runtime::spawn(sync::relay_client::start(app_state, LOCAL_WS_PORT));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth_cmd::auth_status,
            commands::auth_cmd::auth_register,
            commands::auth_cmd::auth_register_with_recovery,
            commands::auth_cmd::auth_login,
            commands::auth_cmd::auth_recovery_question,
            commands::auth_cmd::auth_reset_password_with_recovery,
            commands::auth_cmd::auth_update_recovery,
            commands::diagnostics_cmd::get_connection_diagnostics,
            commands::pairing_cmd::generate_pairing_qr,
            commands::pairing_cmd::consume_pairing,
            commands::pairing_cmd::list_paired_devices,
            commands::pairing_cmd::delete_paired_device,
            commands::pairing_cmd::disconnect_phone,
            commands::pairing_cmd::request_device_reconnect,
            commands::relay_cmd::relay_status,
            commands::relay_cmd::relay_enable,
            commands::relay_cmd::relay_disable,
            commands::relay_cmd::relay_set_url,
            commands::relay_cmd::relay_set_auto_connect,
            commands::settings_cmd::get_settings,
            commands::settings_cmd::set_lock_timeout_minutes,
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
            commands::windows_setup_cmd::run_windows_first_run_setup,
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

#[cfg(test)]
mod tls_provider_tests {
    /// The upgrade to rustls 0.23 made this a runtime panic on a worker thread
    /// rather than a compile error, and every existing test still passed. This one
    /// fails if the choice is ever dropped again.
    #[test]
    fn a_tls_provider_is_chosen_before_any_connection_is_made() {
        super::install_crypto_provider();
        assert!(
            rustls::crypto::CryptoProvider::get_default().is_some(),
            "no TLS provider installed; every TLS connection would panic"
        );
        // Building a client config is the operation that panicked in the field.
        let _ = rustls::ClientConfig::builder()
            .with_root_certificates(rustls::RootCertStore::empty())
            .with_no_client_auth();
    }
}
