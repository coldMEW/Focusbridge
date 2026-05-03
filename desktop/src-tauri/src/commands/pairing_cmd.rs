use crate::db::store;
use crate::pairing::cert_manager::generate_self_signed;
use crate::pairing::device_store::PairingSession;
use crate::pairing::qr_generator::{make_qr, QrOutput, QrPayload};
use crate::state::AppState;
use rand::RngCore;
use std::net::UdpSocket;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn random_hex_256() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn local_ipv4() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

#[tauri::command]
pub fn generate_pairing_qr(state: tauri::State<'_, AppState>) -> Result<QrOutput, String> {
    let cert = generate_self_signed("focusbridge-desktop").map_err(|e| e.to_string())?;
    let device_id = Uuid::new_v4().to_string();
    let pairing_key = random_hex_256();
    let endpoint = format!("{}:9173", local_ipv4());
    let expires_at = now_millis() + 5 * 60 * 1000;
    let payload = QrPayload {
        v: 1,
        mode: "local".into(),
        endpoint: endpoint.clone(),
        relay_url: None,
        device_id: device_id.clone(),
        pairing_key: pairing_key.clone(),
        cert_fingerprint: cert.fingerprint_sha256_hex.clone(),
    };
    state.set_pairing(PairingSession {
        device_id,
        pairing_key,
        cert_fingerprint: cert.fingerprint_sha256_hex,
        expires_at,
    });
    make_qr(&payload, expires_at).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn consume_pairing(
    device_id: String,
    pairing_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let Some(session) = state.current_pairing() else {
        return Err("no active pairing session".into());
    };
    if session.device_id != device_id || session.pairing_key != pairing_key {
        return Err("pairing session mismatch".into());
    }
    store::save_pairing(
        &state.db_path,
        "Android phone",
        &session.device_id,
        &session.pairing_key,
        "",
        &session.cert_fingerprint,
    )
    .map_err(|e| e.to_string())
}
