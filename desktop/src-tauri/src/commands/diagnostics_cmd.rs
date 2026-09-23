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
    /// When a dropped connection began to be re-established, while it still
    /// is; the interface shows "Reconnecting" rather than a disconnection.
    pub reconnecting_since: Option<i64>,
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
        reconnecting_since: state.reconnecting_since(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_millis() as i64)
                .unwrap_or(0),
        ),
    }
}
