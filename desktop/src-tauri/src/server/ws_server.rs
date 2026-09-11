use crate::db::store;
use crate::desktop_notifications;
use crate::server::heartbeat::PeerHeartbeat;
use crate::server::socket_io::{send_frame_before, send_text, service_while, PendingMessages};
use crate::state::AppState;
use anyhow::{Context, Result};
use focusbridge_core::attach::may_attach;
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

/// Ceiling on connections being handled at once, authenticated or not.
pub const MAX_UNAUTHENTICATED_CONNECTIONS: usize = 24;

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

    // This listener is reachable from the whole local network, and each
    // connection costs a TLS handshake, buffers and a task for up to the
    // authentication deadline. Without a ceiling, anything on the network can
    // exhaust the machine by opening sockets and never authenticating. One phone
    // and the loopback relay bridge need two; the rest is slack for reconnects.
    let slots = Arc::new(tokio::sync::Semaphore::new(MAX_UNAUTHENTICATED_CONNECTIONS));
    loop {
        let (stream, peer) = listener.accept().await.context("accept websocket tcp")?;
        let Ok(slot) = slots.clone().try_acquire_owned() else {
            // Dropping is deliberate: queueing would move the exhaustion rather
            // than refuse it, and a real phone retries within seconds.
            warn!(peer = %peer, "refused connection; too many are already open");
            drop(stream);
            continue;
        };
        let state = state.clone();
        let app = app.clone();
        let tls_acceptor = tls_acceptor.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = handle_connection(stream, peer, state, app, tls_acceptor).await {
                warn!(peer = %peer, error = %err, "websocket connection ended");
            }
            drop(slot);
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
                    send_frame_before(&mut ws, Message::Ping(token.into()), heartbeat.deadline()).await?;
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
                send_frame_before(&mut ws, Message::Text(body.into()), heartbeat.deadline())
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
        if active_pairing_key.is_some() && msg.is_text() {
            // Liveness is the phone's silence, not the transport pong alone. A
            // handset streaming notifications is plainly connected even when its
            // pong is late, and hanging the session on that one reply is what
            // dropped healthy connections at random. Only application frames
            // count here: a pong is a challenge, and it is answered in
            // `acknowledge` or not at all.
            heartbeat.mark_alive(Instant::now());
            state.mark_heartbeat(now_ms() as i64);
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
                // Feature 1, the user's: may this PC take a saved phone that
                // arrives without anyone having asked for it -- at startup, or
                // any other time it turns up on its own? Off, only a phone
                // presenting the code currently on screen may attach; picking a
                // saved phone grants a one-time allowance instead.
                //
                // This asks nothing about how the phone arrived. The check used to
                // apply only to the relay, so the same phone the user had told this
                // PC not to reattach to did exactly that over the local network,
                // every launch. The setting is about which phone may attach, not
                // about which cable it came down.
                let auto_first_connection =
                    crate::sync::relay_api::auto_connect(&state.db_path).unwrap_or(true);
                // True when this phone presents the key of the code currently on
                // screen. That means "scanned since the disconnect" only because
                // disconnecting retires the code: otherwise the phone just let go
                // of still holds the live key and walks straight back in.
                let holds_the_code_on_screen = state
                    .current_pairing()
                    .is_some_and(|session| session.pairing_key == expected_key)
                    && state.pairing_code_is_live();
                // Every input to the decision, because "why did it let that phone
                // in" is otherwise unanswerable from the outside, and this rule
                // has been wrong more than once.
                let paused = state.is_paused();
                // Which phone this is, needed before the decision rather than
                // after it: "the user already connected this one" is part of the
                // rule now, not just bookkeeping for the saved-device list.
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
                // Feature 2, the backend's: this is the phone the user already
                // connected in this run, coming back because the transport under
                // it dropped. Putting that back is plumbing, not a preference --
                // the user said yes to this phone once and is not asked again
                // every time a radio blinks. Enforcing feature 1 per socket made
                // it do this job too, and badly: the single-use allowance was
                // spent on the first attach, so every hiccup was permanent.
                let resuming_the_users_connection = state.is_approved_for_this_run(device_id);
                info!(
                    peer = %peer,
                    paused,
                    auto_first_connection,
                    holds_the_code_on_screen,
                    resuming_the_users_connection,
                    "deciding whether this phone may attach"
                );
                // Scanning the code that is on screen is the user asking for this
                // phone, so it ends the disconnect. Nothing else does.
                if holds_the_code_on_screen && paused {
                    info!("a phone scanned the code on screen; the disconnect is over");
                    state.resume();
                }
                if !may_attach(
                    paused,
                    auto_first_connection,
                    holds_the_code_on_screen,
                    resuming_the_users_connection,
                    || {
                        let granted = state.take_known_phone_allowance();
                        if granted {
                            info!("allowed: the user asked for this phone by name");
                        }
                        granted
                    },
                ) {
                    warn!(
                        peer = %peer,
                        paused = state.is_paused(),
                        "refused a phone; it was disconnected here or automatic reconnection is off"
                    );
                    state.note_known_phone_refused();
                    send_text(
                        &mut ws,
                        // "not_accepting" is a decision that can change: the user
                        // may pick this phone under previous connections, or turn
                        // automatic reconnection on. The phone should keep waiting
                        // to be asked for rather than give up.
                        r#"{"version":1,"type":"AUTH_FAILED","payload":{"reason":"not_accepting"}}"#
                            .into(),
                    )
                    .await
                    .ok();
                    break;
                }
                info!(peer = %peer, "phone authenticated");
                // However it got in -- scanning the code, or being picked by
                // name -- the user has now said yes to this phone. It may come
                // back on its own when the transport drops under it, until the
                // user disconnects or this PC restarts.
                state.approve_phone_for_this_run(device_id);
                active_pairing_key = Some(expected_key.clone());
                state.set_phone_sender(outbound_tx.clone());
                // The relay bridge reaches this server over loopback, so a loopback
                // peer is a cross-network session rather than a LAN one. Reporting
                // it as "wss" would tell the user they are on their local network.
                let via_relay = peer.ip().is_loopback();
                state.mark_transport(if via_relay { "relay" } else { "wss" });
                let device_name = envelope
                    .payload
                    .get("deviceName")
                    .and_then(|value| value.as_str())
                    .unwrap_or("Android phone");
                // This PC's own certificate, not the pairing session's copy of it.
                // They are the same value, but the session is cleared when the
                // user disconnects, and reading it from there would have written
                // an empty fingerprint over the saved device -- quietly breaking
                // the pinning that "reconnect a known phone" depends on.
                let cert_fingerprint = state.cert.fingerprint_sha256_hex.clone();
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
                    if state.desktop_notifications_enabled() {
                        desktop_notifications::show_connection_notification(&app, true);
                    }
                    // Tracked whether or not it was shown, so turning the popups
                    // back on mid-session cannot produce a second "connected".
                    notified_connected = true;
                }
            }
            IncomingDecision::AuthFailed(reason) => {
                // Distinguish a pairing this PC cannot recognise from a phone that
                // simply got something wrong. A phone whose credentials this PC no
                // longer holds can never succeed by trying again -- it has to be
                // paired afresh -- and saying so is the difference between "scan
                // the code again" and a phone that sits at "connecting" forever.
                let unknown_pairing = reason.contains("missing session key")
                    || reason.contains("unknown pairing");
                warn!(peer = %peer, reason = %reason, "phone auth failed");
                send_text(&mut ws,
                    if unknown_pairing {
                        r#"{"version":1,"type":"AUTH_FAILED","payload":{"reason":"unknown_pairing"}}"#.into()
                    } else {
                        r#"{"version":1,"type":"AUTH_FAILED","payload":{"reason":"rejected"}}"#.into()
                    },
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
                send_frame_before(&mut ws, Message::Text(body.into()), heartbeat.deadline())
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
        if notified_connected && state.desktop_notifications_enabled() {
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
            // The other half of the phone's capture line. Between them, "nothing
            // arrives on the PC" can be answered without guessing: either the
            // phone never captured it, or it never got here. No message content
            // is recorded, only where it came from.
            info!(app = %row.package_name, stored = !existed, "notification received");
            if !current() {
                return Ok(None);
            }
            // Only pushed into the interface once it is entitled to show it.
            // Nothing is missed by staying quiet: the row is stored regardless,
            // and the inbox loads the stored history from the database when a
            // phone attaches. (It used to load on mount instead -- before the
            // vault is open, so the load was refused and the history never
            // appeared at all.)
            if state.vault_is_unlocked() {
                app.emit("focusbridge://notification", &row)?;
            }
            if !existed && current() {
                // Stored either way, so nothing is lost and the inbox is complete
                // the moment the vault opens; but not displayed, because a desktop
                // notification puts the message on screen for anyone walking past.
                if !state.vault_is_unlocked() {
                    info!("held a notification off screen: the vault is locked");
                } else if !state.desktop_notifications_enabled() {
                    // The user asked for no popups. The message is stored and is
                    // in the inbox already; only the popup is skipped.
                    info!("held a notification off screen: desktop popups are off");
                } else {
                    desktop_notifications::show_phone_notification(app, &row);
                }
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

/// Whether a phone that has authenticated may actually attach.
///
/// This is the whole of "reconnect to the last phone automatically" and of
/// "disconnect means disconnect", in one place, asking nothing about how the
/// phone arrived. Both rules had been written into the relay path only, so both
/// were silently inert over the local network -- which is the path a phone on the
/// same Wi-Fi takes first, and therefore the one the user actually sees.
///
/// A paused PC is one the user has disconnected. It accepts nothing on its own,
/// whatever the setting says; only scanning a code or asking for that phone by
/// name brings it back, and both of those resume it in the same breath.
///
/// The allowance is taken only when it is needed, because taking it consumes it.
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
    send_frame_before(ws, Message::Text(body.into()), deadline)
        .await
        .context("send notification ack")?;
    Ok(())
}
