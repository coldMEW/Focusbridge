use crate::commands::pairing_cmd::local_ipv4_candidates;
use crate::state::AppState;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSnapshot {
    pub connected: bool,
    pub connected_at: Option<i64>,
    pub active_transport: String,
    pub lan_port: u16,
    pub endpoint_candidates: Vec<String>,
    pub certificate_fingerprint: String,
    pub pairing_active: bool,
    pub last_heartbeat_at: Option<i64>,
    pub last_auth_failure: Option<String>,
    pub last_disconnect_reason: Option<String>,
}

#[tauri::command]
pub fn get_connection_diagnostics(state: tauri::State<'_, AppState>) -> DiagnosticsSnapshot {
    let diagnostics = state.diagnostics();
    DiagnosticsSnapshot {
        connected: diagnostics.connected,
        connected_at: diagnostics.connected_at,
        active_transport: diagnostics.active_transport,
        lan_port: 9173,
        endpoint_candidates: local_ipv4_candidates(),
        certificate_fingerprint: state.cert.fingerprint_sha256_hex.clone(),
        pairing_active: state.current_pairing().is_some(),
        last_heartbeat_at: diagnostics.last_heartbeat_at,
        last_auth_failure: diagnostics.last_auth_failure,
        last_disconnect_reason: diagnostics.last_disconnect_reason,
    }
}
