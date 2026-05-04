use crate::pairing::device_store::PairingSession;
use focusbridge_core::cert::GeneratedCert;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
    pub cert: Arc<GeneratedCert>,
    pairing: Arc<Mutex<Option<PairingSession>>>,
    phone_sender: Arc<Mutex<Option<UnboundedSender<String>>>>,
}

impl AppState {
    pub fn new(db_path: PathBuf, cert: GeneratedCert) -> Self {
        Self {
            db_path,
            cert: Arc::new(cert),
            pairing: Arc::new(Mutex::new(None)),
            phone_sender: Arc::new(Mutex::new(None)),
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
    }

    pub fn clear_phone_sender(&self) {
        *self
            .phone_sender
            .lock()
            .expect("phone sender lock poisoned") = None;
    }

    pub fn send_to_phone(&self, message: String) -> bool {
        self.phone_sender
            .lock()
            .expect("phone sender lock poisoned")
            .as_ref()
            .map(|sender| sender.send(message).is_ok())
            .unwrap_or(false)
    }
}
