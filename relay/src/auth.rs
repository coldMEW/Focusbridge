use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

const PASSWORD_ITERATIONS: u32 = 210_000;
const MIN_PASSWORD_LEN: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Android,
    Desktop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordHash {
    pub salt_hex: String,
    pub hash_hex: String,
    pub iterations: u32,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("invalid pairing key")]
    InvalidPairingKey,
    #[error("invalid role")]
    InvalidRole,
    #[error("empty pairing key")]
    EmptyKey,
    #[error("password is too weak")]
    WeakPassword,
    #[error("invalid email")]
    InvalidEmail,
    #[error("invalid token")]
    InvalidToken,
    #[error("expired token")]
    ExpiredToken,
}

pub fn parse_role(role: &str) -> Result<Role, AuthError> {
    match role {
        "android" | "phone" => Ok(Role::Android),
        "desktop" => Ok(Role::Desktop),
        _ => Err(AuthError::InvalidRole),
    }
}

/// Constant-time compare via subtle-less fallback: compares length first, then byte-by-byte.
/// For relay purposes this is sufficient — pairing key is 256-bit random.
pub fn verify_pairing_key(provided: &str, expected: &str) -> Result<(), AuthError> {
    if provided.is_empty() {
        return Err(AuthError::EmptyKey);
    }
    if provided.len() != expected.len() {
        return Err(AuthError::InvalidPairingKey);
    }
    let mut diff: u8 = 0;
    for (a, b) in provided.bytes().zip(expected.bytes()) {
        diff |= a ^ b;
    }
    if diff == 0 {
        Ok(())
    } else {
        Err(AuthError::InvalidPairingKey)
    }
}

pub fn normalize_email(email: &str) -> Result<String, AuthError> {
    let normalized = email.trim().to_ascii_lowercase();
    let (local, domain) = normalized.split_once('@').ok_or(AuthError::InvalidEmail)?;
    if local.is_empty()
        || domain.is_empty()
        || domain.starts_with('.')
        || domain.ends_with('.')
        || !domain.contains('.')
        || normalized.chars().any(char::is_whitespace)
    {
        return Err(AuthError::InvalidEmail);
    }
    Ok(normalized)
}

pub fn create_password_hash(password: &str) -> Result<PasswordHash, AuthError> {
    validate_password(password)?;
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    Ok(hash_password_with_salt(
        password,
        &salt,
        PASSWORD_ITERATIONS,
    ))
}

pub fn verify_password(password: &str, expected: &PasswordHash) -> Result<bool, AuthError> {
    let salt = hex::decode(&expected.salt_hex).map_err(|_| AuthError::InvalidToken)?;
    let actual = hash_password_with_salt(password, &salt, expected.iterations);
    Ok(constant_time_eq(
        actual.hash_hex.as_bytes(),
        expected.hash_hex.as_bytes(),
    ))
}

pub fn create_session_token(
    user_id: &str,
    secret: &str,
    now_epoch_secs: u64,
    ttl_secs: u64,
) -> String {
    let expires_at = now_epoch_secs.saturating_add(ttl_secs);
    let user = URL_SAFE_NO_PAD.encode(user_id.as_bytes());
    let unsigned = format!("v1.{user}.{expires_at}");
    let signature = sign(unsigned.as_bytes(), secret.as_bytes());
    format!("{unsigned}.{signature}")
}

pub fn verify_session_token(
    token: &str,
    secret: &str,
    now_epoch_secs: u64,
) -> Result<String, AuthError> {
    let parts = token.split('.').collect::<Vec<_>>();
    if parts.len() != 4 || parts[0] != "v1" {
        return Err(AuthError::InvalidToken);
    }
    let unsigned = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
    let expected = sign(unsigned.as_bytes(), secret.as_bytes());
    if !constant_time_eq(expected.as_bytes(), parts[3].as_bytes()) {
        return Err(AuthError::InvalidToken);
    }
    let expires_at = parts[2]
        .parse::<u64>()
        .map_err(|_| AuthError::InvalidToken)?;
    if now_epoch_secs > expires_at {
        return Err(AuthError::ExpiredToken);
    }
    let user_id = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| AuthError::InvalidToken)?;
    String::from_utf8(user_id).map_err(|_| AuthError::InvalidToken)
}

fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.len() < MIN_PASSWORD_LEN {
        return Err(AuthError::WeakPassword);
    }
    Ok(())
}

fn hash_password_with_salt(password: &str, salt: &[u8], iterations: u32) -> PasswordHash {
    let mut out = [0u8; 32];
    pbkdf2::<Hmac<Sha256>>(password.as_bytes(), salt, iterations, &mut out)
        .expect("pbkdf2 output length is fixed");
    PasswordHash {
        salt_hex: hex::encode(salt),
        hash_hex: hex::encode(out),
        iterations,
    }
}

fn sign(message: &[u8], secret: &[u8]) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts arbitrary key lengths");
    mac.update(message);
    hex::encode(mac.finalize().into_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_parses() {
        assert_eq!(parse_role("android").unwrap(), Role::Android);
        assert_eq!(parse_role("phone").unwrap(), Role::Android);
        assert_eq!(parse_role("desktop").unwrap(), Role::Desktop);
    }

    #[test]
    fn key_matches() {
        let key = "a".repeat(64);
        assert!(verify_pairing_key(&key, &key).is_ok());
    }

    #[test]
    fn key_rejects_mismatch() {
        assert_eq!(
            verify_pairing_key("abcd", "wxyz"),
            Err(AuthError::InvalidPairingKey)
        );
    }

    #[test]
    fn key_rejects_empty() {
        assert_eq!(verify_pairing_key("", "anything"), Err(AuthError::EmptyKey));
    }

    #[test]
    fn key_rejects_length_diff() {
        assert_eq!(
            verify_pairing_key("abc", "abcd"),
            Err(AuthError::InvalidPairingKey)
        );
    }

    #[test]
    fn password_hash_verifies_original_password_only() {
        let hash = create_password_hash("correct horse battery staple").unwrap();

        assert!(verify_password("correct horse battery staple", &hash).unwrap());
        assert!(!verify_password("wrong horse battery staple", &hash).unwrap());
    }

    #[test]
    fn password_policy_rejects_weak_values() {
        assert_eq!(create_password_hash("short"), Err(AuthError::WeakPassword));
    }

    #[test]
    fn email_policy_rejects_invalid_values() {
        assert_eq!(
            normalize_email("not-an-email"),
            Err(AuthError::InvalidEmail)
        );
        assert_eq!(
            normalize_email(" USER@Example.COM ").unwrap(),
            "user@example.com"
        );
    }

    #[test]
    fn signed_token_roundtrips_and_rejects_tampering() {
        let secret = "test-secret";
        let token = create_session_token("user_123", secret, 1_000, 3_600);

        assert_eq!(
            verify_session_token(&token, secret, 2_000).unwrap(),
            "user_123"
        );

        let mut tampered = token.clone();
        let last = tampered.pop().unwrap();
        tampered.push(if last == '0' { '1' } else { '0' });
        assert_eq!(
            verify_session_token(&tampered, secret, 2_000),
            Err(AuthError::InvalidToken)
        );
    }

    #[test]
    fn signed_token_expires() {
        let token = create_session_token("user_123", "test-secret", 1_000, 10);

        assert_eq!(
            verify_session_token(&token, "test-secret", 1_011),
            Err(AuthError::ExpiredToken)
        );
    }
}
