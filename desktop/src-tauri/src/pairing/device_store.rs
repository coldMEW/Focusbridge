use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingSession {
    pub device_id: String,
    pub pairing_key: String,
    pub cert_fingerprint: String,
    pub expires_at: i64,
}
