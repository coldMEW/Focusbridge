use crate::pairing::device_store::PairingSession;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
    pairing: Arc<Mutex<Option<PairingSession>>>,
}

impl AppState {
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            pairing: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_pairing(&self, session: PairingSession) {
        *self.pairing.lock().expect("pairing lock poisoned") = Some(session);
    }

    pub fn current_pairing(&self) -> Option<PairingSession> {
        self.pairing.lock().expect("pairing lock poisoned").clone()
    }
}
