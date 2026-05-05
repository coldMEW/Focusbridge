use crate::db::store;
use crate::pairing::device_store::PairingSession;
use crate::pairing::qr_generator::{make_qr, QrOutput, QrPayload};
use crate::state::AppState;
use rand::RngCore;
use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::process::Command;
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

fn route_ipv4() -> Option<Ipv4Addr> {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .ok()
        .and_then(|addr| match addr.ip() {
            IpAddr::V4(ip) if is_pairable_ipv4(ip) => Some(ip),
            _ => None,
        })
}

fn is_pairable_ipv4(ip: Ipv4Addr) -> bool {
    !(ip.is_loopback() || ip.is_link_local() || ip.is_unspecified())
}

fn parse_ipv4_token(token: &str) -> Option<Ipv4Addr> {
    let trimmed = token
        .trim()
        .trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
    let ip: Ipv4Addr = trimmed.parse().ok()?;
    is_pairable_ipv4(ip).then_some(ip)
}

#[cfg(target_os = "windows")]
fn command_ipv4_candidates() -> Vec<Ipv4Addr> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let Ok(output) = Command::new("ipconfig")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .filter(|line| line.contains("IPv4"))
        .filter_map(|line| line.split(':').nth(1))
        .filter_map(parse_ipv4_token)
        .collect()
}

#[cfg(not(target_os = "windows"))]
fn command_ipv4_candidates() -> Vec<Ipv4Addr> {
    let output = Command::new("hostname").arg("-I").output();
    let Ok(output) = output else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .filter_map(parse_ipv4_token)
        .collect()
}

pub(crate) fn local_ipv4_candidates() -> Vec<String> {
    let mut candidates = BTreeSet::new();

    if let Some(ip) = route_ipv4() {
        candidates.insert(format!("ws://{ip}:9173"));
    }

    for ip in command_ipv4_candidates() {
        candidates.insert(format!("ws://{ip}:9173"));
    }

    if candidates.is_empty() {
        candidates.insert("ws://127.0.0.1:9173".to_string());
    }

    candidates.into_iter().collect()
}

#[tauri::command]
pub fn generate_pairing_qr(state: tauri::State<'_, AppState>) -> Result<QrOutput, String> {
    let now = now_millis();
    let existing = state
        .current_pairing()
        .filter(|session| session.expires_at > now + 30_000);
    let device_id = existing
        .as_ref()
        .map(|session| session.device_id.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let pairing_key = existing
        .as_ref()
        .map(|session| session.pairing_key.clone())
        .unwrap_or_else(random_hex_256);
    let endpoint_candidates = local_ipv4_candidates();
    let endpoint = endpoint_candidates
        .first()
        .cloned()
        .unwrap_or_else(|| "ws://127.0.0.1:9173".to_string());
    let expires_at = existing
        .as_ref()
        .map(|session| session.expires_at)
        .unwrap_or_else(|| now + 5 * 60 * 1000);
    let payload = QrPayload {
        v: 1,
        mode: "local".into(),
        endpoint: endpoint.clone(),
        endpoint_candidates,
        relay_url: None,
        device_pair_id: None,
        device_id: device_id.clone(),
        pairing_key: pairing_key.clone(),
        cert_fingerprint: state.cert.fingerprint_sha256_hex.clone(),
    };
    state.set_pairing(PairingSession {
        device_id,
        pairing_key,
        cert_fingerprint: state.cert.fingerprint_sha256_hex.clone(),
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
