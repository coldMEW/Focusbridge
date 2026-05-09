use crate::db::store;
use crate::desktop_notifications;
use crate::state::AppState;
use anyhow::{Context, Result};
use focusbridge_core::handler::{handle_envelope, IncomingDecision};
use focusbridge_core::protocol::{Envelope, MessageType};
use focusbridge_core::secure_envelope::{decrypt_payload, encrypt_envelope};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::{accept_async, WebSocketStream};
use tracing::{error, info, warn};

pub struct WsServerConfig {
    pub bind: SocketAddr,
}

pub async fn start(cfg: WsServerConfig, state: AppState, app: AppHandle) {
    if let Err(err) = run(cfg, state, app).await {
        error!(error = %err, "desktop websocket server stopped");
    }
}

async fn run(cfg: WsServerConfig, state: AppState, app: AppHandle) -> Result<()> {
    let listener = TcpListener::bind(cfg.bind)
        .await
        .with_context(|| format!("bind desktop websocket server on {}", cfg.bind))?;
    let tls_cfg =
        crate::server::tls::rustls_config_from_pem(&state.cert.cert_pem, &state.cert.key_pem)?;
    let tls_acceptor = TlsAcceptor::from(std::sync::Arc::new(tls_cfg));
    info!(bind = %cfg.bind, "desktop wss server listening");

    loop {
        let (stream, peer) = listener.accept().await.context("accept websocket tcp")?;
        let state = state.clone();
        let app = app.clone();
        let tls_acceptor = tls_acceptor.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = handle_connection(stream, peer, state, app, tls_acceptor).await {
                warn!(peer = %peer, error = %err, "websocket connection ended");
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    state: AppState,
    app: AppHandle,
    tls_acceptor: TlsAcceptor,
) -> Result<()> {
    let mut first = [0u8; 1];
    let peeked = stream
        .peek(&mut first)
        .await
        .context("peek websocket tcp")?;
    if peeked > 0 && first[0] == 0x16 {
        let tls_stream = tls_acceptor.accept(stream).await.context("accept tls")?;
        let ws = accept_async(tls_stream)
            .await
            .context("accept secure websocket")?;
        state.mark_transport("wss");
        handle_websocket(ws, peer, state, app).await
    } else {
        warn!(peer = %peer, "accepting legacy plaintext websocket; ask user to refresh QR for WSS pinning");
        let ws = accept_async(stream)
            .await
            .context("accept legacy websocket")?;
        state.mark_transport("ws_legacy");
        handle_websocket(ws, peer, state, app).await
    }
}

async fn handle_websocket<S>(
    mut ws: WebSocketStream<S>,
    peer: SocketAddr,
    state: AppState,
    app: AppHandle,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<String>();
    app.emit("focusbridge://connection", "CONNECTING")?;
    let mut active_pairing_key: Option<String> = None;
    let mut notified_connected = false;
    let mut last_phone_seen_at = now_ms() as i64;
    let mut stale_check = interval(Duration::from_secs(5));

    loop {
        let msg = tokio::select! {
            _ = stale_check.tick() => {
                if active_pairing_key.is_some() && now_ms() as i64 - last_phone_seen_at > 180_000 {
                    state.mark_stale_connection("phone heartbeat timeout");
                    app.emit("focusbridge://connection", "DISCONNECTED")?;
                    if notified_connected {
                        desktop_notifications::show_connection_notification(&app, false);
                        notified_connected = false;
                    }
                    active_pairing_key = None;
                    break;
                }
                continue;
            }
            outbound = outbound_rx.recv() => {
                let Some(outbound) = outbound else {
                    break;
                };
                let body = match active_pairing_key.as_deref() {
                    Some(key) => encrypt_envelope(key, &outbound).context("encrypt outbound envelope")?,
                    None => outbound,
                };
                ws.send(tokio_tungstenite::tungstenite::Message::Text(body))
                    .await
                    .context("send outbound websocket message")?;
                continue;
            }
            inbound = ws.next() => {
                let Some(inbound) = inbound else {
                    break;
                };
                inbound.context("read websocket message")?
            }
        };
        if !msg.is_text() {
            continue;
        }
        let text = msg.into_text().context("read websocket text")?;
        last_phone_seen_at = now_ms() as i64;
        let mut envelope: Envelope =
            serde_json::from_str(&text).context("parse focusbridge envelope")?;
        let expected_key = if let Some(key) = active_pairing_key.as_deref() {
            key.to_string()
        } else {
            expected_pairing_key_for_envelope(&state, &envelope).unwrap_or_default()
        };
        if envelope.r#type == MessageType::Encrypted {
            let decrypted = decrypt_payload(&expected_key, &envelope.payload)?;
            envelope =
                serde_json::from_str(&decrypted).context("parse encrypted focusbridge envelope")?;
        }

        match handle_envelope(&envelope, &expected_key) {
            IncomingDecision::AuthAccepted => {
                info!(peer = %peer, "phone authenticated");
                active_pairing_key = Some(expected_key.clone());
                last_phone_seen_at = now_ms() as i64;
                state.set_phone_sender(outbound_tx.clone());
                let qr_device_id = envelope
                    .payload
                    .get("deviceId")
                    .and_then(|value| value.as_str())
                    .unwrap_or("android-phone");
                let device_id = envelope
                    .payload
                    .get("phoneInstallId")
                    .and_then(|value| value.as_str())
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(qr_device_id);
                let device_name = envelope
                    .payload
                    .get("deviceName")
                    .and_then(|value| value.as_str())
                    .unwrap_or("Android phone");
                let cert_fingerprint = state
                    .current_pairing()
                    .map(|session| session.cert_fingerprint)
                    .unwrap_or_default();
                let endpoint = peer.ip().to_string();
                store::mark_pairing_connected(
                    &state.db_path,
                    device_name,
                    device_id,
                    &expected_key,
                    &endpoint,
                    &cert_fingerprint,
                )?;
                ws.send(tokio_tungstenite::tungstenite::Message::Text(
                    format!(
                        r#"{{"version":1,"type":"AUTH_OK","payload":{{"serverTime":{},"config":{{"heartbeatInterval":15000,"heartbeatTimeout":180000,"maxMessageSize":1048576}}}}}}"#,
                        now_ms()
                    ),
                ))
                .await
                .context("send auth ok")?;
                if let Ok(message) = store::rules_update_envelope(&state.db_path) {
                    let _ = state.send_to_phone(message);
                }
                app.emit("focusbridge://connection", "CONNECTED")?;
                if !notified_connected {
                    desktop_notifications::show_connection_notification(&app, true);
                    notified_connected = true;
                }
            }
            IncomingDecision::AuthFailed(reason) => {
                warn!(peer = %peer, reason = %reason, "phone auth failed");
                state.mark_auth_failed(&reason);
                ws.send(tokio_tungstenite::tungstenite::Message::Text(
                    r#"{"version":1,"type":"AUTH_FAILED","payload":{}}"#.into(),
                ))
                .await
                .ok();
                app.emit("focusbridge://connection", "DISCONNECTED")?;
            }
            IncomingDecision::StoreNotification(payload) => {
                let existed = payload
                    .get("id")
                    .and_then(|value| value.as_str())
                    .map(|id| store::notification_exists(&state.db_path, id))
                    .transpose()?
                    .unwrap_or(false);
                let is_new = !existed;
                let row = store::upsert_notification(&state.db_path, &payload)?;
                app.emit("focusbridge://notification", &row)?;
                if is_new {
                    desktop_notifications::show_phone_notification(&app, &row);
                }
                send_notification_ack(&mut ws, active_pairing_key.as_deref(), &row.id).await?;
            }
            IncomingDecision::StoreBatch(payload) => {
                if let Some(items) = payload.get("notifications").and_then(|v| v.as_array()) {
                    for item in items {
                        let existed = item
                            .get("id")
                            .and_then(|value| value.as_str())
                            .map(|id| store::notification_exists(&state.db_path, id))
                            .transpose()?
                            .unwrap_or(false);
                        let is_new = !existed;
                        let row = store::upsert_notification(&state.db_path, item)?;
                        app.emit("focusbridge://notification", &row)?;
                        if is_new {
                            desktop_notifications::show_phone_notification(&app, &row);
                        }
                        send_notification_ack(&mut ws, active_pairing_key.as_deref(), &row.id)
                            .await?;
                    }
                }
            }
            IncomingDecision::RemoveNotification(id) => {
                store::dismiss_notification(&state.db_path, &id)?;
                app.emit("focusbridge://dismissal", id)?;
            }
            IncomingDecision::RemoveBatch(ids) => {
                for id in ids {
                    store::dismiss_notification(&state.db_path, &id)?;
                    app.emit("focusbridge://dismissal", id)?;
                }
            }
            IncomingDecision::PingReceived => {
                let heartbeat_at = now_ms() as i64;
                state.mark_heartbeat(heartbeat_at);
                let pong = format!(
                    r#"{{"version":1,"type":"PONG","payload":{{"serverTime":{}}}}}"#,
                    heartbeat_at
                );
                let body = match active_pairing_key.as_deref() {
                    Some(key) => encrypt_envelope(key, &pong).context("encrypt pong envelope")?,
                    None => pong,
                };
                ws.send(tokio_tungstenite::tungstenite::Message::Text(body))
                    .await
                    .context("send pong")?;
                app.emit("focusbridge://status", "PING")?;
            }
            IncomingDecision::StatusUpdate(payload) => {
                app.emit("focusbridge://phone-status", payload)?;
            }
            IncomingDecision::AppInventory(payload) => {
                let rules = store::save_app_inventory(&state.db_path, &payload)?;
                app.emit("focusbridge://app-rules", rules)?;
            }
            IncomingDecision::RulesAck(payload) => {
                app.emit("focusbridge://rules-ack", payload)?;
            }
            IncomingDecision::ManualDisconnect => {
                state.mark_manual_disconnect();
                store::mark_pairings_disconnected(&state.db_path)?;
                app.emit("focusbridge://connection", "DISCONNECTED")?;
                if notified_connected {
                    desktop_notifications::show_connection_notification(&app, false);
                    notified_connected = false;
                }
                ws.close(None).await.ok();
                active_pairing_key = None;
                break;
            }
            IncomingDecision::Unknown => {}
        }
    }

    state.clear_phone_sender_if_current(&outbound_tx);
    if active_pairing_key.is_some() {
        app.emit("focusbridge://connection", "DISCONNECTED")?;
        if notified_connected {
            desktop_notifications::show_connection_notification(&app, false);
        }
    }
    Ok(())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn expected_pairing_key_for_envelope(state: &AppState, envelope: &Envelope) -> Option<String> {
    if envelope.r#type != MessageType::Auth {
        return state.current_pairing().map(|session| session.pairing_key);
    }

    let presented_key = envelope
        .payload
        .get("pairingKey")
        .and_then(|value| value.as_str())?;
    let qr_device_id = envelope
        .payload
        .get("deviceId")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let stable_device_id = envelope
        .payload
        .get("phoneInstallId")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(qr_device_id);

    if let Some(session) = state.current_pairing() {
        if session.pairing_key == presented_key && session.device_id == qr_device_id {
            return Some(session.pairing_key);
        }
    }

    store::saved_pairing_key_for_device(&state.db_path, stable_device_id, presented_key)
        .ok()
        .flatten()
        .or_else(|| {
            store::saved_pairing_key_for_device(&state.db_path, qr_device_id, presented_key)
                .ok()
                .flatten()
        })
}

async fn send_notification_ack<S>(
    ws: &mut WebSocketStream<S>,
    pairing_key: Option<&str>,
    id: &str,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let ack = serde_json::json!({
        "version": 1,
        "type": "NOTIFICATION_ACK",
        "payload": {
            "id": id,
            "accepted": true,
            "serverTime": now_ms()
        }
    })
    .to_string();
    let body = match pairing_key {
        Some(key) => encrypt_envelope(key, &ack).context("encrypt notification ack envelope")?,
        None => ack,
    };
    ws.send(tokio_tungstenite::tungstenite::Message::Text(body))
        .await
        .context("send notification ack")?;
    Ok(())
}
