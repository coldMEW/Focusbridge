use crate::db::store;
use crate::desktop_notifications;
use crate::server::heartbeat::PeerHeartbeat;
use crate::server::socket_io::{send_frame_before, send_text, service_while, PendingMessages};
use crate::state::AppState;
use anyhow::{Context, Result};
use focusbridge_core::handler::{handle_envelope, IncomingDecision};
use focusbridge_core::protocol::{Envelope, MessageType};
use focusbridge_core::secure_envelope::{decrypt_payload, encrypt_envelope};
use futures_util::StreamExt;
use std::net::SocketAddr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::{interval, sleep_until, timeout, Duration, Instant};
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
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
    let peeked = timeout(Duration::from_secs(10), stream.peek(&mut first))
        .await
        .context("connection preface timeout")?
        .context("peek websocket tcp")?;
    if peeked > 0 && first[0] == 0x16 {
        let tls_stream = timeout(Duration::from_secs(10), tls_acceptor.accept(stream))
            .await
            .context("TLS handshake timeout")?
            .context("accept tls")?;
        let ws = timeout(Duration::from_secs(10), accept_async(tls_stream))
            .await
            .context("WebSocket handshake timeout")?
            .context("accept secure websocket")?;
        handle_websocket(ws, peer, state, app).await
    } else {
        anyhow::bail!("plaintext pairing is disabled; update Android and scan a new QR")
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
    let mut active_pairing_key: Option<String> = None;
    let mut notified_connected = false;
    let auth_deadline = Instant::now() + Duration::from_secs(10);
    let mut heartbeat = PeerHeartbeat::new(Instant::now());
    let mut stale_check = interval(Duration::from_secs(1));
    let mut pending = PendingMessages::default();
    let work_session = WorkSession {
        state: &state,
        app: &app,
        sender: &outbound_tx,
    };

    let result: Result<()> = async {
      loop {
        if active_pairing_key.is_some() && !state.is_current_phone_sender(&outbound_tx) {
            pending.clear();
        }
        let msg = tokio::select! {
            _ = sleep_until(heartbeat.deadline()), if active_pairing_key.is_some() => {
                anyhow::bail!("phone heartbeat timed out");
            }
            _ = sleep_until(auth_deadline), if active_pairing_key.is_none() => {
                anyhow::bail!("phone authentication timed out");
            }
            _ = sleep_until(heartbeat.wake_at()), if active_pairing_key.is_some() => {
                if let Some(token) = heartbeat.probe(Instant::now()) {
                    send_frame_before(&mut ws, Message::Ping(token), heartbeat.deadline()).await?;
                }
                continue;
            }
            _ = stale_check.tick() => {
                // Let a queued UNPAIR drain before closing a revoked session.
                if active_pairing_key.is_some()
                    && !state.is_current_phone_sender(&outbound_tx)
                    && outbound_rx.is_empty()
                {
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
                send_frame_before(&mut ws, Message::Text(body), heartbeat.deadline())
                    .await
                    .context("send outbound websocket message")?;
                continue;
            }
            inbound = async {
                if let Some(frame) = pending.pop_front() { Some(Ok(frame)) } else { ws.next().await }
            } => {
                let Some(inbound) = inbound else {
                    break;
                };
                inbound.context("read websocket message")?
            }
        };
        if active_pairing_key.is_some() && !state.is_current_phone_sender(&outbound_tx) {
            // An old socket may still receive data after replacement or manual pause.
            // It must not mutate notifications, inventory, or current diagnostics.
            continue;
        }
        if !msg.is_text() {
            match msg {
                Message::Close(_) => break,
                Message::Pong(payload) if active_pairing_key.is_some()
                    && heartbeat.acknowledge(&payload, Instant::now()) => {
                    state.mark_heartbeat(now_ms() as i64);
                }
                _ => {}
            }
            continue;
        }
        let text = msg.into_text().context("read websocket text")?;
        let mut envelope: Envelope =
            serde_json::from_str(&text).context("parse focusbridge envelope")?;
        // Application messages must never run before this socket authenticates.
        if active_pairing_key.is_none() && envelope.r#type != MessageType::Auth {
            timeout(Duration::from_secs(2), ws.close(None)).await.ok();
            break;
        }
        if active_pairing_key.is_some() && envelope.r#type == MessageType::Auth {
            timeout(Duration::from_secs(2), ws.close(None)).await.ok();
            break;
        }
        let expected_key = if let Some(key) = active_pairing_key.as_deref() {
            key.to_string()
        } else {
            expected_pairing_key_for_envelope(&state, &envelope).unwrap_or_default()
        };
        if envelope.r#type == MessageType::Encrypted {
            let decrypted = decrypt_payload(&expected_key, &envelope.payload)?;
            envelope =
                serde_json::from_str(&decrypted).context("parse encrypted focusbridge envelope")?;
            if envelope.r#type == MessageType::Auth {
                break;
            }
        }

        match handle_envelope(&envelope, &expected_key) {
            IncomingDecision::AuthAccepted => {
                info!(peer = %peer, "phone authenticated");
                active_pairing_key = Some(expected_key.clone());
                state.set_phone_sender(outbound_tx.clone());
                // The relay bridge reaches this server over loopback, so a loopback
                // peer is a cross-network session rather than a LAN one. Reporting
                // it as "wss" would tell the user they are on their local network.
                let via_relay = peer.ip().is_loopback();
                state.mark_transport(if via_relay { "relay" } else { "wss" });
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
                // A loopback address is the bridge, not the phone's real address;
                // recording it would put 127.0.0.1 in the paired-device list.
                let endpoint = if via_relay {
                    "relay".to_string()
                } else {
                    peer.ip().to_string()
                };
                store::mark_pairing_connected(
                    &state.db_path,
                    device_name,
                    device_id,
                    &expected_key,
                    &endpoint,
                    &cert_fingerprint,
                )?;
                send_text(&mut ws,
                    format!(
                        r#"{{"version":1,"type":"AUTH_OK","payload":{{"serverTime":{},"config":{{"heartbeatInterval":15000,"heartbeatTimeout":180000,"maxMessageSize":1048576}}}}}}"#,
                        now_ms()
                    ),
                )
                .await
                .context("send auth ok")?;
                if !state.is_current_phone_sender(&outbound_tx) {
                    continue;
                }
                heartbeat = PeerHeartbeat::new(Instant::now());
                state.mark_heartbeat(now_ms() as i64);
                if let Ok(message) = store::rules_update_envelope(&state.db_path) {
                    let _ = outbound_tx.send(message);
                }
                app.emit("focusbridge://connection", "CONNECTED")?;
                if !notified_connected {
                    desktop_notifications::show_connection_notification(&app, true);
                    notified_connected = true;
                }
            }
            IncomingDecision::AuthFailed(reason) => {
                warn!(peer = %peer, reason = %reason, "phone auth failed");
                send_text(&mut ws,
                    r#"{"version":1,"type":"AUTH_FAILED","payload":{}}"#.into(),
                )
                .await
                .ok();
                break;
            }
            IncomingDecision::StoreBatch(mut payload) => {
                if let Some(items) = payload.get_mut("notifications").and_then(|v| v.as_array_mut()) {
                    for item in std::mem::take(items) {
                        if !work_session.apply(&mut ws, &mut heartbeat, &mut pending,
                            IncomingDecision::StoreNotification(item), active_pairing_key.as_deref()).await? {
                            break;
                        }
                    }
                }
            }
            IncomingDecision::RemoveBatch(ids) => {
                for id in ids {
                    if !work_session.apply(&mut ws, &mut heartbeat, &mut pending,
                        IncomingDecision::RemoveNotification(id), active_pairing_key.as_deref()).await? {
                        break;
                    }
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
                send_frame_before(&mut ws, Message::Text(body), heartbeat.deadline())
                .await
                .context("send pong")?;
                if state.is_current_phone_sender(&outbound_tx) {
                    app.emit("focusbridge://status", "PING")?;
                }
            }
            work @ (IncomingDecision::StoreNotification(_) | IncomingDecision::RemoveNotification(_)
                | IncomingDecision::StatusUpdate(_) | IncomingDecision::AppInventory(_)
                | IncomingDecision::RulesAck(_)) => {
                work_session.apply(&mut ws, &mut heartbeat, &mut pending, work,
                    active_pairing_key.as_deref()).await?;
            }
            IncomingDecision::ManualDisconnect => {
                break;
            }
            IncomingDecision::Unknown => {}
        }
    }
      Ok(())
    }.await;

    pending.clear();
    let reason = result
        .as_ref()
        .err()
        .map(|error| error.to_string())
        .unwrap_or_else(|| "socket closed".into());
    if state.clear_phone_sender_if_current_with_reason(&outbound_tx, &reason) {
        store::mark_pairings_disconnected(&state.db_path)?;
        app.emit("focusbridge://connection", "DISCONNECTED")?;
        if notified_connected {
            desktop_notifications::show_connection_notification(&app, false);
        }
    }
    timeout(Duration::from_secs(2), ws.close(None)).await.ok();
    result
}

struct WorkSession<'a> {
    state: &'a AppState,
    app: &'a AppHandle,
    sender: &'a mpsc::UnboundedSender<String>,
}

// Dropping a timed-out work future cannot stop an already-running SQLite call, but it can
// prevent its result from publishing events or ACKs after this socket has been abandoned.
struct WorkPermit(Arc<AtomicBool>);

impl Drop for WorkPermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl WorkSession<'_> {
    async fn apply<S>(
        &self,
        ws: &mut WebSocketStream<S>,
        heartbeat: &mut PeerHeartbeat,
        pending: &mut PendingMessages,
        decision: IncomingDecision,
        pairing_key: Option<&str>,
    ) -> Result<bool>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let permit = WorkPermit(Arc::new(AtomicBool::new(true)));
        let allowed = permit.0.clone();
        let state = self.state.clone();
        let app = self.app.clone();
        let sender = self.sender.clone();
        let work = async move {
            tokio::task::spawn_blocking(move || {
                apply_work_item(decision, &state, &app, &sender, &allowed)
            })
            .await
            .context("application work failed")?
        };
        let result = service_while(
            ws,
            heartbeat,
            pending,
            work,
            || self.state.is_current_phone_sender(self.sender),
            || {
                if self.state.is_current_phone_sender(self.sender) {
                    self.state.mark_heartbeat(now_ms() as i64);
                }
            },
        )
        .await;
        drop(permit);
        // Let the outer loop drain a queued desktop UNPAIR, but never start another work item.
        if !self.state.is_current_phone_sender(self.sender) {
            return Ok(false);
        }
        if let Some(id) = result? {
            send_notification_ack(ws, pairing_key, &id, heartbeat.deadline()).await?;
        }
        Ok(true)
    }
}

fn apply_work_item(
    decision: IncomingDecision,
    state: &AppState,
    app: &AppHandle,
    sender: &mpsc::UnboundedSender<String>,
    allowed: &AtomicBool,
) -> Result<Option<String>> {
    let current = || allowed.load(Ordering::Acquire) && state.is_current_phone_sender(sender);
    if !current() {
        return Ok(None);
    }
    match decision {
        IncomingDecision::StoreNotification(payload) => {
            let existed = payload
                .get("id")
                .and_then(|value| value.as_str())
                .map(|id| store::notification_exists(&state.db_path, id))
                .transpose()?
                .unwrap_or(false);
            if !current() {
                return Ok(None);
            }
            let row = store::upsert_notification(&state.db_path, &payload)?;
            if !current() {
                return Ok(None);
            }
            app.emit("focusbridge://notification", &row)?;
            if !existed && current() {
                desktop_notifications::show_phone_notification(app, &row);
            }
            return Ok(Some(row.id));
        }
        IncomingDecision::RemoveNotification(id) => {
            store::dismiss_notification(&state.db_path, &id)?;
            if current() {
                app.emit("focusbridge://dismissal", id)?;
            }
        }
        IncomingDecision::StatusUpdate(payload) => {
            app.emit("focusbridge://phone-status", payload)?;
        }
        IncomingDecision::RulesAck(payload) => {
            app.emit("focusbridge://rules-ack", payload)?;
        }
        IncomingDecision::AppInventory(payload) => {
            let rules = store::save_app_inventory(&state.db_path, &payload)?;
            if !current() {
                return Ok(None);
            }
            app.emit("focusbridge://app-rules", rules)?;
            let message = store::rules_update_envelope(&state.db_path)?;
            if current() {
                let _ = sender.send(message);
            }
        }
        _ => anyhow::bail!("unexpected queued application work"),
    }
    Ok(None)
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
        if session.expires_at > now_ms() as i64
            && session.pairing_key == presented_key
            && session.device_id == qr_device_id
        {
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
    deadline: Instant,
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
    send_frame_before(ws, Message::Text(body), deadline)
        .await
        .context("send notification ack")?;
    Ok(())
}
