use crate::pairing::device_store::PairingSession;
use focusbridge_core::cert::GeneratedCert;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

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
}

impl AppState {
    pub fn new(db_path: PathBuf, cert: GeneratedCert) -> Self {
        Self {
            db_path,
            cert: Arc::new(cert),
            pairing: Arc::new(Mutex::new(None)),
            phone_sender: Arc::new(Mutex::new(None)),
            diagnostics: Arc::new(Mutex::new(ConnectionDiagnostics::default())),
        }
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
            diag.last_disconnect_reason = None;
        });
    }

    pub fn clear_phone_sender(&self) {
        *self
            .phone_sender
            .lock()
            .expect("phone sender lock poisoned") = None;
    }

    pub fn clear_phone_sender_if_current(&self, sender: &UnboundedSender<String>) {
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
                diag.last_disconnect_reason = Some("socket closed".into());
            });
        }
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
