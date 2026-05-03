use crate::db::store;
use crate::state::AppState;
use anyhow::{Context, Result};
use focusbridge_core::handler::{handle_envelope, IncomingDecision};
use focusbridge_core::protocol::Envelope;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tauri::{AppHandle, Emitter};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
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
    info!(bind = %cfg.bind, "desktop websocket server listening");

    loop {
        let (stream, peer) = listener.accept().await.context("accept websocket tcp")?;
        let state = state.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = handle_connection(stream, peer, state, app).await {
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
) -> Result<()> {
    let mut ws = accept_async(stream).await.context("accept websocket")?;
    app.emit("focusbridge://connection", "CONNECTING")?;

    while let Some(msg) = ws.next().await {
        let msg = msg.context("read websocket message")?;
        if !msg.is_text() {
            continue;
        }
        let text = msg.into_text().context("read websocket text")?;
        let envelope: Envelope =
            serde_json::from_str(&text).context("parse focusbridge envelope")?;
        let expected_key = state
            .current_pairing()
            .map(|p| p.pairing_key)
            .unwrap_or_default();

        match handle_envelope(&envelope, &expected_key) {
            IncomingDecision::AuthAccepted => {
                info!(peer = %peer, "phone authenticated");
                ws.send(tokio_tungstenite::tungstenite::Message::Text(
                    r#"{"version":1,"type":"AUTH_OK","payload":{}}"#.into(),
                ))
                .await
                .context("send auth ok")?;
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
                app.emit("focusbridge://notification", row)?;
            }
            IncomingDecision::StoreBatch(payload) => {
                if let Some(items) = payload.get("notifications").and_then(|v| v.as_array()) {
                    for item in items {
                        let row = store::upsert_notification(&state.db_path, item)?;
                        app.emit("focusbridge://notification", row)?;
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
            IncomingDecision::Unknown => {}
        }
    }

    app.emit("focusbridge://connection", "DISCONNECTED")?;
    Ok(())
}
