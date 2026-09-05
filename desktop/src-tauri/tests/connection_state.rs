#![allow(dead_code)]

#[path = "../src/pairing/device_store.rs"]
pub mod device_store;
mod pairing {
    pub use crate::device_store;
}
#[path = "../src/server/socket_io.rs"]
mod socket_io;
#[path = "../src/state.rs"]
mod state;

use focusbridge_core::cert::generate_self_signed;
use state::AppState;
use tokio::sync::mpsc;

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
