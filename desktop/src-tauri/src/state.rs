use crate::pairing::device_store::PairingSession;
use focusbridge_core::cert::GeneratedCert;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Notify;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDiagnostics {
    pub connected: bool,
    pub connected_at: Option<i64>,
    pub active_transport: String,
    pub last_heartbeat_at: Option<i64>,
    pub last_auth_failure: Option<String>,
    pub last_disconnect_reason: Option<String>,
}

impl Default for ConnectionDiagnostics {
    fn default() -> Self {
        Self {
            connected: false,
            connected_at: None,
            active_transport: "none".into(),
            last_heartbeat_at: None,
            last_auth_failure: None,
            last_disconnect_reason: None,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
    pub cert: Arc<GeneratedCert>,
    pairing: Arc<Mutex<Option<PairingSession>>>,
    phone_sender: Arc<Mutex<Option<UnboundedSender<String>>>>,
    diagnostics: Arc<Mutex<ConnectionDiagnostics>>,
    /// Set when the user asks for a specific phone rather than waiting for the
    /// automatic connection, and cleared once a session is established.
    relay_requested: Arc<AtomicBool>,
    relay_wake: Arc<Notify>,
    relay_idle_reason: Arc<Mutex<Option<String>>>,
    enrollment_armed: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(db_path: PathBuf, cert: GeneratedCert) -> Self {
        Self {
            db_path,
            cert: Arc::new(cert),
            pairing: Arc::new(Mutex::new(None)),
            phone_sender: Arc::new(Mutex::new(None)),
            diagnostics: Arc::new(Mutex::new(ConnectionDiagnostics::default())),
            relay_requested: Arc::new(AtomicBool::new(false)),
            relay_wake: Arc::new(Notify::new()),
            relay_idle_reason: Arc::new(Mutex::new(None)),
            enrollment_armed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Allows the next phone to enroll, replacing any pinned identity.
    ///
    /// Armed only when the user generates a fresh pairing code, and cleared as
    /// soon as a phone takes it, so a known phone authenticates on every later
    /// connection rather than silently re-enrolling.
    pub fn arm_enrollment(&self) {
        self.enrollment_armed.store(true, Ordering::Release);
    }

    pub fn enrollment_armed(&self) -> bool {
        self.enrollment_armed.load(Ordering::Acquire)
    }

    pub fn disarm_enrollment(&self) {
        self.enrollment_armed.store(false, Ordering::Release);
    }

    /// True when this is a new reason, so it is worth saying out loud.
    pub fn note_relay_idle(&self, reason: &str) -> bool {
        let mut current = self
            .relay_idle_reason
            .lock()
            .expect("relay idle lock poisoned");
        if current.as_deref() == Some(reason) {
            return false;
        }
        *current = Some(reason.to_string());
        true
    }

    /// Asks the relay client to connect now, even when automatic connection is
    /// off. This is how "reconnect this phone" reaches a phone on another
    /// network: neither device can dial the other, so both meet at the relay.
    pub fn request_relay_connection(&self, reason: &str) {
        // Say who asked. Without this, an unexpected connection is impossible to
        // attribute, and the reconnection preference looks like it is ignored.
        tracing::info!(reason, "relay connection requested");
        // The next idle reason is worth repeating: circumstances just changed.
        *self
            .relay_idle_reason
            .lock()
            .expect("relay idle lock poisoned") = None;
        self.relay_requested.store(true, Ordering::Release);
        // notify_one stores a permit when nobody is parked yet. notify_waiters
        // does not, so a request arriving in the gap between the supervisor
        // testing the flag and parking was dropped, leaving it asleep forever
        // with the request still pending: pressing the button did nothing.
        self.relay_wake.notify_one();
    }

    pub fn take_relay_request(&self) -> bool {
        self.relay_requested.swap(false, Ordering::AcqRel)
    }

    pub fn relay_connection_requested(&self) -> bool {
        self.relay_requested.load(Ordering::Acquire)
    }

    /// Waits until someone asks for a connection.
    /// Waits for a request, rechecking periodically.
    ///
    /// The timeout is a safety net, not the mechanism: a permit wakes this
    /// immediately. It exists so that no future lost wakeup can strand the
    /// supervisor indefinitely, which is a bad failure because it looks exactly
    /// like the feature being broken.
    pub async fn await_relay_request(&self) {
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.relay_wake.notified(),
        )
        .await;
    }

    pub fn set_pairing(&self, session: PairingSession) {
        *self.pairing.lock().expect("pairing lock poisoned") = Some(session);
    }

    pub fn current_pairing(&self) -> Option<PairingSession> {
        self.pairing.lock().expect("pairing lock poisoned").clone()
    }

    pub fn set_phone_sender(&self, sender: UnboundedSender<String>) {
        *self
            .phone_sender
            .lock()
            .expect("phone sender lock poisoned") = Some(sender);
        self.update_diagnostics(|diag| {
            diag.connected = true;
            diag.connected_at = Some(now_ms_i64());
            diag.last_heartbeat_at = None;
            diag.last_auth_failure = None;
            diag.last_disconnect_reason = None;
        });
    }

    pub fn clear_phone_sender(&self) {
        *self
            .phone_sender
            .lock()
            .expect("phone sender lock poisoned") = None;
    }

    pub fn is_current_phone_sender(&self, sender: &UnboundedSender<String>) -> bool {
        self.phone_sender
            .lock()
            .expect("phone sender lock poisoned")
            .as_ref()
            .map(|active| active.same_channel(sender))
            .unwrap_or(false)
    }

    pub fn mark_manual_disconnect(&self) {
        self.clear_phone_sender();
        self.update_diagnostics(|diag| {
            diag.connected = false;
            diag.connected_at = None;
            diag.active_transport = "none".into();
            diag.last_disconnect_reason = Some("manual disconnect".into());
        });
    }

    pub fn clear_phone_sender_if_current(&self, sender: &UnboundedSender<String>) -> bool {
        self.clear_phone_sender_if_current_with_reason(sender, "socket closed")
    }

    pub fn clear_phone_sender_if_current_with_reason(
        &self,
        sender: &UnboundedSender<String>,
        reason: &str,
    ) -> bool {
        let mut current = self
            .phone_sender
            .lock()
            .expect("phone sender lock poisoned");
        if current
            .as_ref()
            .map(|active| active.same_channel(sender))
            .unwrap_or(false)
        {
            *current = None;
            self.update_diagnostics(|diag| {
                diag.connected = false;
                diag.connected_at = None;
                diag.active_transport = "none".into();
                diag.last_disconnect_reason = Some(reason.into());
            });
            return true;
        }
        false
    }

    pub fn send_to_phone(&self, message: String) -> bool {
        self.phone_sender
            .lock()
            .expect("phone sender lock poisoned")
            .as_ref()
            .map(|sender| sender.send(message).is_ok())
            .unwrap_or(false)
    }

    pub fn mark_transport(&self, transport: &str) {
        self.update_diagnostics(|diag| {
            diag.active_transport = transport.to_string();
            diag.last_disconnect_reason = None;
        });
    }

    pub fn mark_heartbeat(&self, at: i64) {
        self.update_diagnostics(|diag| {
            diag.last_heartbeat_at = Some(at);
        });
    }

    pub fn mark_auth_failed(&self, reason: &str) {
        self.update_diagnostics(|diag| {
            diag.connected = false;
            diag.connected_at = None;
            diag.last_auth_failure = Some(reason.to_string());
            diag.last_disconnect_reason = Some("authentication failed".into());
        });
    }

    pub fn mark_stale_connection(&self, reason: &str) {
        self.update_diagnostics(|diag| {
            diag.connected = false;
            diag.connected_at = None;
            diag.active_transport = "none".into();
            diag.last_disconnect_reason = Some(reason.to_string());
        });
    }

    pub fn diagnostics(&self) -> ConnectionDiagnostics {
        self.diagnostics
            .lock()
            .expect("diagnostics lock poisoned")
            .clone()
    }

    fn update_diagnostics(&self, update: impl FnOnce(&mut ConnectionDiagnostics)) {
        let mut diagnostics = self.diagnostics.lock().expect("diagnostics lock poisoned");
        update(&mut diagnostics);
    }
}

fn now_ms_i64() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}
