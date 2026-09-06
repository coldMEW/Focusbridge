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
    let now = tokio::time::Instant::now();
    let mut health = heartbeat::PeerHeartbeat::new(now);
    health.probe(now).unwrap();
    assert!(health.deadline() <= now + std::time::Duration::from_secs(6));
    assert!(health
        .probe(now + std::time::Duration::from_secs(3))
        .is_none());
    assert!(health.deadline() <= now + std::time::Duration::from_secs(6));
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
    socket_io::send_frame_before(&mut server, Message::Ping(token.clone()), health.deadline())
        .await
        .unwrap();
    assert_eq!(
        phone.next().await.unwrap().unwrap(),
        Message::Ping(token.clone())
    );
    phone.flush().await.unwrap();
    let pong = tokio::time::timeout_at(health.deadline(), server.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(pong, Message::Pong(token.clone()));
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
            Message::Text("x".repeat(1024)),
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
    socket_io::send_frame_before(&mut server, Message::Ping(token), health.deadline())
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
    let now = tokio::time::Instant::now();
    let mut health = heartbeat::PeerHeartbeat::new(now);
    let deadline = health.deadline();
    assert!(health.probe(deadline).is_none());
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
        .push_back(Message::Text("x".repeat(socket_io::MAX_PENDING_BYTES)))
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

#[tokio::test]
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
                Message::Text(text) => assert_eq!(text, "ack"),
                _ => (),
            }
        }
    });
    let mut health = heartbeat::PeerHeartbeat::new(tokio::time::Instant::now());
    let mut pending = socket_io::PendingMessages::default();
    let mut acknowledged = 0;
    for _ in 0..24 {
        socket_io::service_while(
            &mut server,
            &mut health,
            &mut pending,
            async {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
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

#[tokio::test]
async fn silent_peer_expires_while_application_work_is_pending() {
    use tokio_tungstenite::{tungstenite::protocol::Role, WebSocketStream};
    let (left, _right) = tokio::io::duplex(1024);
    let mut server = WebSocketStream::from_raw_socket(left, Role::Server, None).await;
    let mut health = heartbeat::PeerHeartbeat::new(tokio::time::Instant::now());
    // A nearly-expired pending probe keeps the test short without changing production timers.
    health.probe(
        tokio::time::Instant::now() - heartbeat::PROBE_TIMEOUT
            + std::time::Duration::from_millis(50),
    );
    let mut pending = socket_io::PendingMessages::default();
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
