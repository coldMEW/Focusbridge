#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Android,
    Desktop,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("invalid pairing key")]
    InvalidPairingKey,
    #[error("invalid role")]
    InvalidRole,
    #[error("empty pairing key")]
    EmptyKey,
}

pub fn parse_role(role: &str) -> Result<Role, AuthError> {
    match role {
        "android" => Ok(Role::Android),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_parses() {
        assert_eq!(parse_role("android").unwrap(), Role::Android);
        assert_eq!(parse_role("desktop").unwrap(), Role::Desktop);
        assert!(parse_role("phone").is_err());
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
}
