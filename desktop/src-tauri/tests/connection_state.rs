#![allow(dead_code)]

#[path = "../src/pairing/device_store.rs"]
pub mod device_store;
mod pairing {
    pub use crate::device_store;
}
#[path = "../src/server/heartbeat.rs"]
mod heartbeat;
#[path = "../src/server/socket_io.rs"]
mod socket_io;
/// The pairing session is persisted through the settings store so it survives a
/// restart. These tests exercise connection ownership, not storage, so the store
/// is stubbed: a real one would need an encrypted database and a key.
mod db {
    pub mod store {
        use std::path::Path;
        pub fn set_setting(_db: &Path, _key: &str, _value: &str) -> anyhow::Result<()> {
            Ok(())
        }
        pub fn get_setting(_db: &Path, _key: &str) -> anyhow::Result<Option<String>> {
            Ok(None)
        }
    }
}
#[path = "../src/state.rs"]
mod state;

use focusbridge_core::cert::generate_self_signed;
use state::AppState;
use tokio::sync::mpsc;

#[test]
fn silent_peer_expires_without_waiting_for_android_application_heartbeat() {
    // The desktop still gives up on its own schedule rather than waiting out
    // Android's application heartbeat. What changed is the size of that
    // schedule: it used to be six seconds, so the first probe a dozing handset
    // answered late dropped a healthy session, while the phone had been told in
    // AUTH_OK that it had a hundred and eighty.
    let now = tokio::time::Instant::now();
    let mut health = heartbeat::PeerHeartbeat::new(now);
    health.probe(now).unwrap();
    assert_eq!(health.deadline(), now + heartbeat::SESSION_TIMEOUT);
    assert!(health.deadline() < now + std::time::Duration::from_secs(180));
    // A probe already in flight is not re-sent, and probing never buys time.
    assert!(health
        .probe(now + std::time::Duration::from_secs(3))
        .is_none());
    assert_eq!(health.deadline(), now + heartbeat::SESSION_TIMEOUT);
}

#[test]
fn unrelated_or_late_pongs_do_not_extend_connection_lifetime() {
    let now = tokio::time::Instant::now();
    let mut health = heartbeat::PeerHeartbeat::new(now);
    let token = health.probe(now).unwrap();
    let deadline = health.deadline();
    assert!(!health.acknowledge(b"unsolicited", now));
    assert!(!health.acknowledge(&token, deadline));
    assert_eq!(health.deadline(), deadline);
}

#[test]
fn responsive_transport_stays_alive_without_android_application_pings() {
    let mut now = tokio::time::Instant::now();
    let mut health = heartbeat::PeerHeartbeat::new(now);
    for _ in 0..200 {
        let token = health.probe(now).unwrap();
        now += std::time::Duration::from_millis(50);
        assert!(health.acknowledge(&token, now));
        assert!(health.deadline() > now + heartbeat::PROBE_INTERVAL);
        assert!(!health.acknowledge(&token, now));
        now += heartbeat::PROBE_INTERVAL;
    }
}

#[tokio::test]
async fn websocket_control_probe_receives_matching_automatic_pong() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{
        tungstenite::{protocol::Role, Message},
        WebSocketStream,
    };
    let (left, right) = tokio::io::duplex(1024);
    let mut server = WebSocketStream::from_raw_socket(left, Role::Server, None).await;
    let mut phone = WebSocketStream::from_raw_socket(right, Role::Client, None).await;
    let mut health = heartbeat::PeerHeartbeat::new(tokio::time::Instant::now());
    let token = health.probe(tokio::time::Instant::now()).unwrap();
    socket_io::send_frame_before(
        &mut server,
        Message::Ping(token.clone().into()),
        health.deadline(),
    )
    .await
    .unwrap();
    assert_eq!(
        phone.next().await.unwrap().unwrap(),
        Message::Ping(token.clone().into())
    );
    phone.flush().await.unwrap();
    let pong = tokio::time::timeout_at(health.deadline(), server.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(pong, Message::Pong(token.clone().into()));
    assert!(health.acknowledge(&token, tokio::time::Instant::now()));
}

fn state() -> AppState {
    AppState::new(
        "unused.sqlite".into(),
        generate_self_signed("test").unwrap(),
    )
}

#[test]
fn new_connection_does_not_inherit_old_heartbeat_or_auth_failure() {
    let state = state();
    state.mark_heartbeat(1);
    state.mark_auth_failed("old attempt");
    let (sender, _receiver) = mpsc::unbounded_channel();
    state.set_phone_sender(sender);
    let diagnostics = state.diagnostics();
    assert!(diagnostics.connected);
    assert_eq!(diagnostics.last_heartbeat_at, None);
    assert_eq!(diagnostics.last_auth_failure, None);
}

#[test]
fn old_socket_cleanup_cannot_disconnect_replacement() {
    let state = state();
    let (old, _old_receiver) = mpsc::unbounded_channel();
    let (new, mut new_receiver) = mpsc::unbounded_channel();
    state.set_phone_sender(old.clone());
    state.set_phone_sender(new.clone());
    assert!(!state.clear_phone_sender_if_current(&old));
    assert!(state.diagnostics().connected);
    assert!(state.send_to_phone("new socket".into()));
    assert_eq!(new_receiver.try_recv().unwrap(), "new socket");
    assert!(state.clear_phone_sender_if_current(&new));
    assert!(!state.diagnostics().connected);
}

#[test]
fn manual_disconnect_stops_outbound_delivery() {
    let state = state();
    let (sender, _receiver) = mpsc::unbounded_channel();
    state.set_phone_sender(sender.clone());
    state.mark_manual_disconnect();
    assert!(!state.send_to_phone("must not send".into()));
    assert!(!state.clear_phone_sender_if_current(&sender));
    assert!(!state.diagnostics().connected);
}

#[test]
fn only_current_socket_is_authorized_to_process_messages() {
    let state = state();
    let (old, _old_receiver) = mpsc::unbounded_channel();
    let (new, _new_receiver) = mpsc::unbounded_channel();
    state.set_phone_sender(old.clone());
    assert!(state.is_current_phone_sender(&old));
    state.set_phone_sender(new.clone());
    assert!(!state.is_current_phone_sender(&old));
    assert!(state.is_current_phone_sender(&new));
    state.mark_manual_disconnect();
    assert!(!state.is_current_phone_sender(&new));
}

#[tokio::test]
async fn socket_write_times_out_when_peer_stops_reading() {
    use tokio_tungstenite::{tungstenite::protocol::Role, WebSocketStream};
    let (stream, _blocked_peer) = tokio::io::duplex(16);
    let mut socket = WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(12),
        socket_io::send_text(&mut socket, "x".repeat(1024)),
    )
    .await
    .expect("socket helper did not enforce its write deadline")
    .expect_err("backpressured writes must not block disconnect forever");
    assert!(error.to_string().contains("timeout"));
}

#[tokio::test]
async fn socket_write_delivers_to_reading_peer() {
    use futures_util::StreamExt;
    use tokio_tungstenite::{tungstenite::protocol::Role, WebSocketStream};
    let (left, right) = tokio::io::duplex(16);
    let mut sender = WebSocketStream::from_raw_socket(left, Role::Server, None).await;
    let mut receiver = WebSocketStream::from_raw_socket(right, Role::Client, None).await;
    let (sent, received) = tokio::join!(
        socket_io::send_text(&mut sender, "delivery".into()),
        receiver.next()
    );
    sent.unwrap();
    assert_eq!(received.unwrap().unwrap().into_text().unwrap(), "delivery");
}

#[tokio::test]
async fn expired_frame_deadline_rejects_even_a_writable_socket() {
    use tokio_tungstenite::{
        tungstenite::{protocol::Role, Message},
        WebSocketStream,
    };
    let (left, _right) = tokio::io::duplex(1024);
    let mut socket = WebSocketStream::from_raw_socket(left, Role::Server, None).await;
    let error = socket_io::send_frame_before(
        &mut socket,
        Message::Text("must not be sent".into()),
        tokio::time::Instant::now() - std::time::Duration::from_millis(1),
    )
    .await
    .expect_err("an expired deadline must reject ready writes too");
    assert!(error.to_string().contains("timeout"));
}

#[tokio::test]
async fn blocked_frame_write_uses_the_supplied_deadline() {
    use tokio_tungstenite::{
        tungstenite::{protocol::Role, Message},
        WebSocketStream,
    };
    let (left, _right) = tokio::io::duplex(16);
    let mut socket = WebSocketStream::from_raw_socket(left, Role::Server, None).await;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        socket_io::send_frame_before(
            &mut socket,
            Message::Text("x".repeat(1024).into()),
            tokio::time::Instant::now() + std::time::Duration::from_millis(50),
        ),
    )
    .await
    .expect("write ignored the heartbeat deadline");
    assert!(result.unwrap_err().to_string().contains("timeout"));
}

#[tokio::test]
async fn slow_work_services_actual_pongs_and_preserves_application_frame_order() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{
        tungstenite::{protocol::Role, Message},
        WebSocketStream,
    };
    let (left, right) = tokio::io::duplex(4096);
    let mut server = WebSocketStream::from_raw_socket(left, Role::Server, None).await;
    let mut phone = WebSocketStream::from_raw_socket(right, Role::Client, None).await;
    let mut health = heartbeat::PeerHeartbeat::new(tokio::time::Instant::now());
    let token = health.probe(tokio::time::Instant::now()).unwrap();
    socket_io::send_frame_before(&mut server, Message::Ping(token.into()), health.deadline())
        .await
        .unwrap();
    phone.send(Message::Text("first".into())).await.unwrap();
    assert!(phone.next().await.unwrap().unwrap().is_ping());
    phone.flush().await.unwrap();
    phone.send(Message::Text("second".into())).await.unwrap();
    let mut pending = socket_io::PendingMessages::default();
    let mut acknowledged = 0;
    socket_io::service_while(
        &mut server,
        &mut health,
        &mut pending,
        async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            Ok(())
        },
        || true,
        || acknowledged += 1,
    )
    .await
    .unwrap();
    assert_eq!(acknowledged, 1, "matching Pong was left unread during work");
    assert_eq!(pending.pop_front(), Some(Message::Text("first".into())));
    assert_eq!(pending.pop_front(), Some(Message::Text("second".into())));
    assert!(pending.pop_front().is_none());
}

#[test]
fn expired_idle_heartbeat_cannot_be_revived_by_a_delayed_probe() {
    // Sending a probe is not evidence of anything, so it must never buy the
    // session time. This used to be enforced by refusing to probe at all past
    // the deadline; it is now structural, because the deadline is measured from
    // when the phone was last heard from and no probe touches that. The probe
    // itself is free to be retried, which is what stops one late pong from
    // ending a live session.
    let now = tokio::time::Instant::now();
    let mut health = heartbeat::PeerHeartbeat::new(now);
    let deadline = health.deadline();
    health.probe(deadline);
    assert_eq!(health.deadline(), deadline);
    health.probe(deadline + std::time::Duration::from_secs(600));
    assert_eq!(health.deadline(), deadline);
}

#[test]
fn pending_application_frames_are_bounded_by_count_and_bytes() {
    use tokio_tungstenite::tungstenite::Message;
    let mut pending = socket_io::PendingMessages::default();
    for _ in 0..socket_io::MAX_PENDING_MESSAGES {
        pending.push_back(Message::Text("x".into())).unwrap();
    }
    assert!(pending.push_back(Message::Text("overflow".into())).is_err());
    pending.clear();
    pending
        .push_back(Message::Text(
            "x".repeat(socket_io::MAX_PENDING_BYTES).into(),
        ))
        .unwrap();
    assert!(pending.push_back(Message::Text("x".into())).is_err());
    pending.pop_front();
    pending
        .push_back(Message::Text("fits again".into()))
        .unwrap();
}

#[tokio::test]
async fn ownership_loss_drops_queued_frames_without_starting_work() {
    use tokio_tungstenite::{
        tungstenite::{protocol::Role, Message},
        WebSocketStream,
    };
    let (left, _right) = tokio::io::duplex(1024);
    let mut server = WebSocketStream::from_raw_socket(left, Role::Server, None).await;
    let mut health = heartbeat::PeerHeartbeat::new(tokio::time::Instant::now());
    let mut pending = socket_io::PendingMessages::default();
    pending
        .push_back(Message::Text("stale work".into()))
        .unwrap();
    let result = socket_io::service_while(
        &mut server,
        &mut health,
        &mut pending,
        async {
            panic!("stale work must not start");
            #[allow(unreachable_code)]
            Ok(())
        },
        || false,
        || panic!("stale session must not update heartbeat"),
    )
    .await;
    assert!(result.is_err());
    assert!(pending.pop_front().is_none());
}

#[tokio::test(start_paused = true)]
async fn slow_batch_longer_than_response_timeout_keeps_servicing_control_frames() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{
        tungstenite::{protocol::Role, Message},
        WebSocketStream,
    };
    let (left, right) = tokio::io::duplex(4096);
    let mut server = WebSocketStream::from_raw_socket(left, Role::Server, None).await;
    let mut phone = WebSocketStream::from_raw_socket(right, Role::Client, None).await;
    let peer = tokio::spawn(async move {
        while let Some(frame) = phone.next().await {
            match frame.unwrap() {
                Message::Ping(_) => phone.flush().await.unwrap(),
                Message::Text(text) => assert_eq!(text.as_str(), "ack"),
                _ => (),
            }
        }
    });
    let mut health = heartbeat::PeerHeartbeat::new(tokio::time::Instant::now());
    let mut pending = socket_io::PendingMessages::default();
    let mut acknowledged = 0;
    // Each work item outlives a single probe, on the paused clock so the test
    // stays instant. Real timers: the probe lapses at 20s and is retried, and
    // the next scheduled probe falls due at 15s -- both inside one batch.
    let batch = heartbeat::PROBE_TIMEOUT + std::time::Duration::from_secs(5);
    for _ in 0..6 {
        socket_io::service_while(
            &mut server,
            &mut health,
            &mut pending,
            async {
                tokio::time::sleep(batch).await;
                Ok(())
            },
            || true,
            || acknowledged += 1,
        )
        .await
        .unwrap();
        socket_io::send_frame_before(&mut server, Message::Text("ack".into()), health.deadline())
            .await
            .unwrap();
    }
    assert!(
        acknowledged >= 3,
        "batch starved control Pongs: {acknowledged}"
    );
    peer.abort();
}

#[tokio::test(start_paused = true)]
async fn silent_peer_expires_while_application_work_is_pending() {
    use tokio_tungstenite::{tungstenite::protocol::Role, WebSocketStream};
    let (left, _right) = tokio::io::duplex(1024);
    let mut server = WebSocketStream::from_raw_socket(left, Role::Server, None).await;
    let mut health = heartbeat::PeerHeartbeat::new(tokio::time::Instant::now());
    health.probe(tokio::time::Instant::now());
    let mut pending = socket_io::PendingMessages::default();
    // Run the session up to the edge of its silence budget on the paused clock,
    // so the test stays instant without shortening the production timers.
    tokio::time::advance(heartbeat::SESSION_TIMEOUT - std::time::Duration::from_millis(50)).await;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(7),
        socket_io::service_while(
            &mut server,
            &mut health,
            &mut pending,
            std::future::pending::<anyhow::Result<()>>(),
            || true,
            || (),
        ),
    )
    .await
    .expect("application work disabled the heartbeat timer");
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("heartbeat timed out"));
}

#[test]
fn a_full_queue_reports_itself_full_before_a_push_can_fail() {
    use tokio_tungstenite::tungstenite::Message;
    let mut pending = socket_io::PendingMessages::default();
    assert!(!pending.is_full());
    for _ in 0..socket_io::MAX_PENDING_MESSAGES {
        pending.push_back(Message::Text("x".into())).unwrap();
    }
    // The caller asks this instead of pushing and failing, so it has to be true
    // by the time a push would be refused.
    assert!(pending.is_full());
    pending.pop_front();
    assert!(!pending.is_full());
}

#[tokio::test]
async fn a_burst_larger_than_the_queue_is_back_pressured_rather_than_fatal() {
    // The bug that actually disconnected people, caught live on the user's own
    // machine. A phone that reconnects flushes the notifications it was holding,
    // and the whole backlog arrives while one of them is being written to the
    // database. Past MAX_PENDING_MESSAGES the queue refused the next frame and
    // the session died with "pending websocket work limit exceeded" -- so the
    // acknowledgements were never sent, the phone kept the notifications
    // pending, and it flushed the identical burst on the next connection. The
    // session died about a second after every authentication, for ever.
    //
    // Reading is what stops now. The frames wait in the socket, TCP closes the
    // phone's window, and nothing is lost.
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{
        tungstenite::{protocol::Role, Message},
        WebSocketStream,
    };

    let sent = socket_io::MAX_PENDING_MESSAGES + 40;
    let (left, right) = tokio::io::duplex(256 * 1024);
    let mut server = WebSocketStream::from_raw_socket(left, Role::Server, None).await;
    let phone = tokio::spawn(async move {
        let mut phone = WebSocketStream::from_raw_socket(right, Role::Client, None).await;
        for index in 0..sent {
            phone
                .send(Message::Text(format!("notification {index}").into()))
                .await
                .unwrap();
        }
        phone.flush().await.unwrap();
        // Hold the socket open; a close would end the read for a different reason.
        std::future::pending::<()>().await;
    });

    let mut health = heartbeat::PeerHeartbeat::new(tokio::time::Instant::now());
    let mut pending = socket_io::PendingMessages::default();
    socket_io::service_while(
        &mut server,
        &mut health,
        &mut pending,
        async {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            Ok(())
        },
        || true,
        || {},
    )
    .await
    .expect("a burst larger than the queue must not end the session");

    // Everything the phone sent is still there: some queued, the rest waiting in
    // the socket because this side stopped reading.
    let mut seen = 0;
    while pending.pop_front().is_some() {
        seen += 1;
    }
    while seen < sent {
        match tokio::time::timeout(std::time::Duration::from_secs(5), server.next()).await {
            Ok(Some(Ok(Message::Text(_)))) => seen += 1,
            Ok(Some(Ok(_))) => continue,
            _ => break,
        }
    }
    assert_eq!(seen, sent, "frames were lost instead of being held back");
    phone.abort();
}
