use crate::db::store;
use crate::pairing::device_store::PairingSession;
use crate::pairing::qr_generator::{make_qr, QrOutput, QrPayload};
use crate::state::AppState;
use crate::sync::{relay_api, relay_identity};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use focusbridge_core::qr::{QrNoise, QrRelay};
use rand::RngCore;
use serde_json::json;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Emitter;
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

/// LAN addresses a phone should try, best first.
///
/// The address the operating system actually routes through leads, because the
/// machine usually also has virtual adapters - Hyper-V, WSL, VirtualBox - whose
/// addresses no phone can reach. Sorting these numerically used to put a
/// 172.x virtual switch ahead of the real 192.168.x address, so every pairing
/// began by waiting out a ten-second connect timeout to a dead endpoint.
pub(crate) fn local_ipv4_candidates() -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();
    let mut push = |ip: Ipv4Addr| {
        let candidate = format!("wss://{ip}:9173");
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    };

    if let Some(ip) = route_ipv4() {
        push(ip);
    }
    // Remaining adapters are still offered, in a stable order, because the
    // routed address is wrong when the phone is on a hotspot this PC serves.
    let mut others = command_ipv4_candidates();
    others.sort();
    for ip in others {
        push(ip);
    }

    if candidates.is_empty() {
        candidates.push("wss://127.0.0.1:9173".to_string());
    }
    candidates
}

/// Builds the pairing code.
///
/// `for_pairing` separates the user actually pairing a phone from the preview
/// that sits beside the inbox. Only the former is a request to be reachable or
/// to let a new phone enroll; the preview renders on every launch, and treating
/// that as intent would quietly override the reconnection preference.
#[tauri::command]
pub fn generate_pairing_qr(
    for_pairing: bool,
    state: tauri::State<'_, AppState>,
) -> Result<QrOutput, String> {
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
    // A brand new session on the pairing screen means the user asked to pair a
    // phone. Re-rendering reuses the existing session and must not re-arm.
    if for_pairing && existing.is_none() {
        state.arm_enrollment();
        // Asking for a fresh code is asking to pair, which resumes this pairing.
        state.resume();
    }
    let endpoint_candidates = local_ipv4_candidates();
    let endpoint = endpoint_candidates
        .first()
        .cloned()
        .unwrap_or_else(|| "wss://127.0.0.1:9173".to_string());
    let expires_at = existing
        .as_ref()
        .map(|session| session.expires_at)
        .unwrap_or_else(|| now + 5 * 60 * 1000);
    // The relay block is included only as a complete, usable set. If anything is
    // missing the QR stays a working LAN pairing rather than advertising a relay
    // the phone could not authenticate to.
    let (relay, noise) = relay_pairing_blocks(&state);
    // A phone that scans this code may only be able to reach this PC through the
    // relay, and it cannot dial the PC directly, so showing the code on the
    // pairing screen is also a request to be present at the relay. That is a
    // deliberate act and overrides the reconnection preference; the inbox
    // preview is not, and must not.
    if relay.is_some() && !state.is_paused() {
        // Wake the relay supervisor so it re-evaluates: a live code means this PC
        // must be waiting where a phone that scans it can reach it. Who is then
        // let in is decided at authentication, not here.
        state.request_relay_connection("a pairing code is on screen");
    }
    let payload = QrPayload {
        v: if relay.is_some() { 2 } else { 1 },
        mode: "local".into(),
        endpoint: endpoint.clone(),
        endpoint_candidates,
        relay_url: None,
        device_pair_id: None,
        device_id: device_id.clone(),
        pairing_key: pairing_key.clone(),
        cert_fingerprint: state.cert.fingerprint_sha256_hex.clone(),
        relay,
        noise,
    };
    state.set_pairing(PairingSession {
        device_id,
        pairing_key,
        cert_fingerprint: state.cert.fingerprint_sha256_hex.clone(),
        expires_at,
    });
    let qr = make_qr(&payload, expires_at).map_err(|e| e.to_string())?;
    // How the code was encoded decides whether a phone can read it at all, and
    // it is invisible once rendered. The length is safe to record; the link
    // itself is pairing material and is not.
    tracing::info!(
        compact = qr.deep_link.starts_with("focusbridge://pair?c="),
        characters = qr.deep_link.len(),
        "pairing code generated"
    );
    Ok(qr)
}

/// Builds the cross-network half of the QR, or `(None, None)` when the relay is
/// not configured or its key material cannot be read. A relay failure must never
/// break LAN pairing, so every error here degrades to a LAN-only QR.
fn relay_pairing_blocks(state: &AppState) -> (Option<QrRelay>, Option<QrNoise>) {
    let Ok(Some(pair)) = relay_api::current_pair(&state.db_path) else {
        return (None, None);
    };
    if pair.expires_at <= now_millis() {
        return (None, None);
    }
    let Ok(Some(secrets)) = relay_identity::pair_secrets(&state.db_path, &pair.pair_id) else {
        return (None, None);
    };
    let Ok(identity) = relay_identity::identity(&state.db_path) else {
        return (None, None);
    };
    (
        Some(QrRelay {
            url: pair.url.clone(),
            account_key: pair.account_key.clone(),
            pair_id: pair.pair_id.clone(),
            // Only the phone's capability leaves this machine.
            capability: pair.phone_capability.clone(),
        }),
        Some(QrNoise {
            desktop_key: B64.encode(identity.public_key()),
            psk: B64.encode(secrets.psk),
        }),
    )
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

#[tauri::command]
pub fn list_paired_devices(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::db::models::PairedDeviceRow>, String> {
    store::list_paired_devices(&state.db_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_paired_device(
    device_id: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<usize, String> {
    let active = store::list_paired_devices(&state.db_path)
        .map_err(|e| e.to_string())?
        .into_iter()
        .any(|device| device.device_id == device_id && device.is_active != 0);
    if active {
        let message = serde_json::to_string(&json!({
            "version": 1,
            "type": "UNPAIR",
            "payload": {
                "reason": "desktop_deleted_pairing"
            }
        }))
        .map_err(|e| e.to_string())?;
        let _ = state.send_to_phone(message);
        state.mark_manual_disconnect();
        app.emit("focusbridge://connection", "DISCONNECTED")
            .map_err(|e| e.to_string())?;
    }
    store::delete_paired_device(&state.db_path, &device_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn disconnect_phone(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let message = serde_json::to_string(&json!({
        "version": 1,
        "type": "UNPAIR",
        "payload": {
            "reason": "manual_disconnect"
        }
    }))
    .map_err(|e| e.to_string())?;
    let _ = state.send_to_phone(message);
    store::mark_pairings_disconnected(&state.db_path).map_err(|e| e.to_string())?;
    state.mark_manual_disconnect();
    app.emit("focusbridge://connection", "DISCONNECTED")
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn request_device_reconnect(
    device_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let message = serde_json::to_string(&json!({
        "version": 1,
        "type": "DESKTOP_ACTION",
        "payload": {
            "action": "reconnect_request",
            "deviceId": device_id,
            "requestedAt": now_millis()
        }
    }))
    .map_err(|e| e.to_string())?;

    if state.send_to_phone(message) {
        return Ok("Asked your phone to reconnect.".into());
    }
    // No live socket to ask. If cross-network sync is set up, join the relay and
    // wait there instead: the phone cannot be dialed directly on another
    // network, but both ends can meet at the relay.
    if relay_api::current_pair(&state.db_path)
        .map_err(|error| error.to_string())?
        .is_some()
    {
        state.resume();
        state.allow_known_phone();
        state.request_relay_connection("the user asked to reconnect a saved phone");
        return Ok("Waiting for your phone to accept. It can be on any network.".into());
    }
    Err("Phone is offline. Open FocusBridge on Android, then scan the QR or paste the manual payload.".into())
}
