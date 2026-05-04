use crate::db::store;
use crate::desktop_notifications;
use crate::state::AppState;
use anyhow::{Context, Result};
use focusbridge_core::handler::{handle_envelope, IncomingDecision};
use focusbridge_core::protocol::{Envelope, MessageType};
use focusbridge_core::secure_envelope::{decrypt_payload, encrypt_envelope};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
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
        handle_websocket(ws, peer, state, app).await
    } else {
        warn!(peer = %peer, "accepting legacy plaintext websocket; ask user to refresh QR for WSS pinning");
        let ws = accept_async(stream)
            .await
            .context("accept legacy websocket")?;
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

    loop {
        let msg = tokio::select! {
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
        let mut envelope: Envelope =
            serde_json::from_str(&text).context("parse focusbridge envelope")?;
        let expected_key = state
            .current_pairing()
            .map(|p| p.pairing_key)
            .unwrap_or_default();
        if envelope.r#type == MessageType::Encrypted {
            let decrypted = decrypt_payload(&expected_key, &envelope.payload)?;
            envelope =
                serde_json::from_str(&decrypted).context("parse encrypted focusbridge envelope")?;
        }

        match handle_envelope(&envelope, &expected_key) {
            IncomingDecision::AuthAccepted => {
                info!(peer = %peer, "phone authenticated");
                active_pairing_key = Some(expected_key.clone());
                state.set_phone_sender(outbound_tx.clone());
                ws.send(tokio_tungstenite::tungstenite::Message::Text(
                    r#"{"version":1,"type":"AUTH_OK","payload":{}}"#.into(),
                ))
                .await
                .context("send auth ok")?;
                if let Ok(message) = store::rules_update_envelope(&state.db_path) {
                    let _ = state.send_to_phone(message);
                }
                app.emit("focusbridge://connection", "CONNECTED")?;
            }
            IncomingDecision::AuthFailed(reason) => {
                warn!(peer = %peer, reason = %reason, "phone auth failed");
                ws.send(tokio_tungstenite::tungstenite::Message::Text(
                    r#"{"version":1,"type":"AUTH_FAILED","payload":{}}"#.into(),
                ))
                .await
                .ok();
                app.emit("focusbridge://connection", "DISCONNECTED")?;
            }
            IncomingDecision::StoreNotification(payload) => {
                let row = store::upsert_notification(&state.db_path, &payload)?;
                app.emit("focusbridge://notification", &row)?;
                desktop_notifications::show_phone_notification(&app, &row);
            }
            IncomingDecision::StoreBatch(payload) => {
                if let Some(items) = payload.get("notifications").and_then(|v| v.as_array()) {
                    for item in items {
                        let row = store::upsert_notification(&state.db_path, item)?;
                        app.emit("focusbridge://notification", &row)?;
                        desktop_notifications::show_phone_notification(&app, &row);
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
            IncomingDecision::Unknown => {}
        }
    }

    state.clear_phone_sender();
    app.emit("focusbridge://connection", "DISCONNECTED")?;
    Ok(())
}
