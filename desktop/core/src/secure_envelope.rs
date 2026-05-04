use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Context, Result};
use base64::prelude::*;
use hkdf::Hkdf;
use rand::RngCore;
use serde_json::{json, Value};
use sha2::Sha256;

const INFO: &[u8] = b"FocusBridge message encryption v1";

fn cipher(pairing_key: &str) -> Result<Aes256Gcm> {
    let hk = Hkdf::<Sha256>::new(Some(b"focusbridge-v1"), pairing_key.as_bytes());
    let mut key = [0u8; 32];
    hk.expand(INFO, &mut key)
        .map_err(|_| anyhow!("derive message key"))?;
    Aes256Gcm::new_from_slice(&key).context("create aes-gcm cipher")
}

pub fn encrypt_envelope(pairing_key: &str, plaintext_envelope: &str) -> Result<String> {
    let cipher = cipher(pairing_key)?;
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext_envelope.as_bytes())
        .map_err(|_| anyhow!("encrypt envelope"))?;
    Ok(json!({
        "version": 1,
        "type": "ENCRYPTED",
        "payload": {
            "alg": "AES-256-GCM",
            "nonce": BASE64_STANDARD.encode(nonce),
            "ciphertext": BASE64_STANDARD.encode(ciphertext)
        }
    })
    .to_string())
}

pub fn decrypt_payload(pairing_key: &str, payload: &Value) -> Result<String> {
    let nonce = payload
        .get("nonce")
        .and_then(Value::as_str)
        .context("missing encrypted nonce")?;
    let ciphertext = payload
        .get("ciphertext")
        .and_then(Value::as_str)
        .context("missing encrypted ciphertext")?;
    let nonce = BASE64_STANDARD.decode(nonce).context("decode nonce")?;
    let ciphertext = BASE64_STANDARD
        .decode(ciphertext)
        .context("decode ciphertext")?;
    let plaintext = cipher(pairing_key)?
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| anyhow!("decrypt envelope"))?;
    String::from_utf8(plaintext).context("encrypted envelope utf8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_envelope_roundtrips() {
        let raw = r#"{"version":1,"type":"PING","payload":{}}"#;
        let encrypted = encrypt_envelope("pairing-secret", raw).unwrap();
        let wrapper: Value = serde_json::from_str(&encrypted).unwrap();
        assert_eq!(wrapper["type"], "ENCRYPTED");
        let decrypted = decrypt_payload("pairing-secret", &wrapper["payload"]).unwrap();
        assert_eq!(decrypted, raw);
    }
}
