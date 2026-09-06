use super::heartbeat::PeerHeartbeat;
use anyhow::{Context, Result};
use futures_util::{FutureExt, SinkExt, StreamExt};
use std::{collections::VecDeque, future::Future};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::{interval, sleep_until, timeout_at, Duration, Instant};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

pub const MAX_PENDING_MESSAGES: usize = 64;
pub const MAX_PENDING_BYTES: usize = 8 * 1024 * 1024;

#[derive(Default)]
pub struct PendingMessages {
    messages: VecDeque<Message>,
    bytes: usize,
}

impl PendingMessages {
    pub fn pop_front(&mut self) -> Option<Message> {
        let frame = self.messages.pop_front()?;
        self.bytes -= frame.len();
        Some(frame)
    }

    pub fn push_back(&mut self, frame: Message) -> Result<()> {
        anyhow::ensure!(
            self.messages.len() < MAX_PENDING_MESSAGES
                && frame.len() <= MAX_PENDING_BYTES.saturating_sub(self.bytes),
            "pending websocket work limit exceeded"
        );
        self.bytes += frame.len();
        self.messages.push_back(frame);
        Ok(())
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.bytes = 0;
    }
}

/// Keep the actual socket reader and probes running while one application work item is pending.
pub async fn service_while<S, F, T>(
    socket: &mut WebSocketStream<S>,
    heartbeat: &mut PeerHeartbeat,
    pending: &mut PendingMessages,
    work: F,
    mut is_current: impl FnMut() -> bool,
    mut on_pong: impl FnMut(),
) -> Result<T>
where
    S: AsyncRead + AsyncWrite + Unpin,
    F: Future<Output = Result<T>>,
{
    tokio::pin!(work);
    let mut ownership_check = interval(Duration::from_millis(250));
    let result = async {
        loop {
            anyhow::ensure!(is_current(), "socket ownership changed");
            anyhow::ensure!(
                Instant::now() < heartbeat.deadline(),
                "phone heartbeat timed out"
            );
            // Poll real reads even when the next work item completes immediately.
            if let Some(frame) = socket.next().now_or_never() {
                accept_during_work(frame, heartbeat, pending, &mut on_pong)?;
            }
            if let Some(token) = heartbeat.probe(Instant::now()) {
                send_frame_before(socket, Message::Ping(token.into()), heartbeat.deadline()).await?;
            }
            tokio::select! {
                biased;
                _ = sleep_until(heartbeat.deadline()) => anyhow::bail!("phone heartbeat timed out"),
                _ = ownership_check.tick() => continue,
                _ = sleep_until(heartbeat.wake_at()) => continue,
                result = &mut work => {
                    anyhow::ensure!(is_current(), "socket ownership changed");
                    return result;
                }
                frame = socket.next() => {
                    anyhow::ensure!(is_current(), "socket ownership changed");
                    accept_during_work(frame, heartbeat, pending, &mut on_pong)?;
                }
            }
        }
    }
    .await;
    if result.is_err() {
        pending.clear();
    }
    result
}

fn accept_during_work(
    frame: Option<std::result::Result<Message, tokio_tungstenite::tungstenite::Error>>,
    heartbeat: &mut PeerHeartbeat,
    pending: &mut PendingMessages,
    on_pong: &mut impl FnMut(),
) -> Result<()> {
    match frame
        .context("socket closed")?
        .context("read websocket during work")?
    {
        Message::Close(_) => anyhow::bail!("socket closed"),
        Message::Pong(payload) if heartbeat.acknowledge(&payload, Instant::now()) => on_pong(),
        frame @ Message::Text(_) => pending.push_back(frame)?,
        // Tungstenite queues the reply to Ping; the next read or write flushes it.
        _ => (),
    }
    Ok(())
}

pub async fn send_text<S>(socket: &mut WebSocketStream<S>, body: String) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    send_frame_before(
        socket,
        Message::Text(body.into()),
        Instant::now() + Duration::from_secs(10),
    )
    .await
}

pub async fn send_frame_before<S>(
    socket: &mut WebSocketStream<S>,
    frame: Message,
    deadline: Instant,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    anyhow::ensure!(Instant::now() < deadline, "websocket write timeout");
    timeout_at(
        deadline.min(Instant::now() + Duration::from_secs(10)),
        socket.send(frame),
    )
    .await
    .context("websocket write timeout")?
    .context("send websocket message")?;
    anyhow::ensure!(Instant::now() < deadline, "websocket write timeout");
    Ok(())
}
