use super::heartbeat::PeerHeartbeat;
use anyhow::{Context, Result};
use futures_util::{FutureExt, SinkExt, StreamExt};
use std::{collections::VecDeque, future::Future};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::{interval, sleep_until, timeout_at, Duration, Instant};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

pub const MAX_PENDING_MESSAGES: usize = 64;
pub const MAX_PENDING_BYTES: usize = 8 * 1024 * 1024;
/// The largest record the protocol allows. A queue with less room than this is
/// treated as full, so the decision to stop reading is never left to the size of
/// whichever frame happens to arrive next.
const LARGEST_RECORD: usize = 1024 * 1024;

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

    /// True when nothing more may be queued.
    ///
    /// The caller stops *reading* the socket rather than pushing and failing, so
    /// a burst becomes back-pressure instead of a dropped session. This is the
    /// bug that actually disconnected people: a phone flushing its backlog after
    /// connecting sends its held notifications at once, and more than
    /// `MAX_PENDING_MESSAGES` of them arriving while one was being written to
    /// the database ended the session with "pending websocket work limit
    /// exceeded". Caught live, one second after a connection was established:
    ///
    /// ```text
    /// notification received app=... stored=true
    /// the phone session ended reason="pending websocket work limit exceeded"
    /// ```
    ///
    /// The limit is a memory bound and stays exactly where it was. What changed
    /// is what happens on reaching it: TCP stops being drained, the phone's
    /// window closes, and it waits -- which is what a full queue is supposed to
    /// mean. Nothing is dropped and nothing is lost.
    pub fn is_full(&self) -> bool {
        self.messages.len() >= MAX_PENDING_MESSAGES
            || self.bytes + LARGEST_RECORD > MAX_PENDING_BYTES
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
            // Stop draining the socket while the queue is full, rather than
            // reading a frame there is no room for and ending the session over
            // it. The work item in flight is what empties the queue, and it is
            // already being polled below.
            // Poll real reads even when the next work item completes immediately.
            if !pending.is_full() {
                if let Some(frame) = socket.next().now_or_never() {
                    accept_during_work(frame, heartbeat, pending, &mut on_pong)?;
                }
            }
            if let Some(token) = heartbeat.probe(Instant::now()) {
                send_frame_before(socket, Message::Ping(token.into()), heartbeat.deadline())
                    .await?;
            }
            // Asked again, deliberately: the read just above may have taken the
            // last slot, and one pass round this loop can otherwise queue twice
            // while having checked for room once. That is not a theoretical
            // race -- the regression test for the burst caught it here.
            let queue_has_room = !pending.is_full();
            tokio::select! {
                biased;
                _ = sleep_until(heartbeat.deadline()) => anyhow::bail!("phone heartbeat timed out"),
                _ = ownership_check.tick() => continue,
                _ = sleep_until(heartbeat.wake_at()) => continue,
                result = &mut work => {
                    anyhow::ensure!(is_current(), "socket ownership changed");
                    return result;
                }
                frame = socket.next(), if queue_has_room => {
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
    let frame = frame
        .context("socket closed")?
        .context("read websocket during work")?;
    match frame {
        Message::Close(_) => anyhow::bail!("socket closed"),
        Message::Pong(payload) if heartbeat.acknowledge(&payload, Instant::now()) => on_pong(),
        frame @ Message::Text(_) => {
            // The phone can only have sent this itself, so work in flight cannot
            // let a session lapse just because a pong is queued behind it.
            heartbeat.mark_alive(Instant::now());
            pending.push_back(frame)?
        }
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
