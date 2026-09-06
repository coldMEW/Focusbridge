//! Pure relay protocol helpers, shared by the provisioning client and the relay
//! transport. Everything here is total and side-effect free so it can be tested
//! directly; the Tauri crate disables its own unit-test harness.

use anyhow::{Context, Result};

/// A pair provisioned by the relay for one desktop and one phone.
///
/// `account_key` and `pair_id` are routing metadata. The capabilities authorize
/// routing only: they cannot decrypt application traffic and are revocable by the
/// account owner without touching device key material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayPair {
    pub url: String,
    pub account_key: String,
    pub pair_id: String,
    pub desktop_capability: String,
    pub phone_capability: String,
    pub expires_at: i64,
}

fn lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn capability(value: &str) -> bool {
    // 32 random bytes, base64url without padding.
    value.len() == 43
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

impl RelayPair {
    /// Rejects anything that is not exactly the shape the relay issues, so a
    /// corrupt or attacker-supplied record can never become a dialed endpoint.
    pub fn is_valid(&self) -> bool {
        self.url.starts_with("https://")
            && self.url.len() > "https://".len()
            && !self.url.contains(char::is_whitespace)
            && lower_hex(&self.account_key, 64)
            && lower_hex(&self.pair_id, 32)
            && capability(&self.desktop_capability)
            && capability(&self.phone_capability)
    }

    /// `wss://host/v1/socket/{accountKey}/{pairId}/{role}` for this pair.
    pub fn socket_url(&self, role: &str) -> String {
        let base = self
            .url
            .trim_end_matches('/')
            .replacen("https://", "wss://", 1);
        format!(
            "{base}/v1/socket/{}/{}/{role}",
            self.account_key, self.pair_id
        )
    }
}

/// Reads the `type` of a relay control frame.
///
/// These frames are the relay's own bounded metadata, never peer data: they carry
/// no application content and an unrecognised or oversized frame yields `None`.
pub fn control_type(text: &str) -> Option<String> {
    if text.len() > 512 {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(text)
        .ok()?
        .get("type")?
        .as_str()
        .map(str::to_string)
}

pub fn pair_id_bytes(pair_id: &str) -> Result<[u8; 16]> {
    anyhow::ensure!(
        lower_hex(pair_id, 32),
        "relay pair identifier must be 32 lowercase hex characters"
    );
    hex::decode(pair_id)
        .context("decode the relay pair identifier")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("relay pair identifier has the wrong length"))
}

pub fn fingerprint_bytes(value: &str) -> Result<[u8; 32]> {
    anyhow::ensure!(
        value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()),
        "certificate fingerprint must be 64 hex characters"
    );
    hex::decode(value)
        .context("decode the certificate fingerprint")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("certificate fingerprint has the wrong length"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> RelayPair {
        RelayPair {
            url: "https://relay.example".into(),
            account_key: "a".repeat(64),
            pair_id: "b".repeat(32),
            desktop_capability: "C".repeat(43),
            phone_capability: "D".repeat(43),
            expires_at: 42,
        }
    }

    #[test]
    fn socket_url_names_the_role_over_tls() {
        assert_eq!(
            pair().socket_url("desktop"),
            format!(
                "wss://relay.example/v1/socket/{}/{}/desktop",
                "a".repeat(64),
                "b".repeat(32)
            )
        );
        let mut trailing = pair();
        trailing.url = "https://relay.example/".into();
        assert_eq!(
            trailing.socket_url("phone"),
            format!(
                "wss://relay.example/v1/socket/{}/{}/phone",
                "a".repeat(64),
                "b".repeat(32)
            )
        );
    }

    #[test]
    fn a_well_formed_pair_is_accepted() {
        assert!(pair().is_valid());
    }

    #[test]
    fn malformed_pairs_are_rejected() {
        type Mutation = (&'static str, fn(&mut RelayPair));
        let cases: Vec<Mutation> = vec![
            ("short account", |p| p.account_key = "a".repeat(63)),
            ("uppercase account", |p| p.account_key = "A".repeat(64)),
            ("non hex account", |p| p.account_key = "z".repeat(64)),
            ("short pair", |p| p.pair_id = "b".repeat(31)),
            ("long pair", |p| p.pair_id = "b".repeat(33)),
            ("short capability", |p| {
                p.desktop_capability = "C".repeat(42)
            }),
            ("bad capability byte", |p| {
                p.phone_capability = format!("{}!", "D".repeat(42))
            }),
            ("empty capability", |p| p.phone_capability = String::new()),
            ("plaintext url", |p| p.url = "http://relay.example".into()),
            ("empty url", |p| p.url = String::new()),
            ("whitespace url", |p| {
                p.url = "https://relay .example".into()
            }),
        ];
        for (name, mutate) in cases {
            let mut candidate = pair();
            mutate(&mut candidate);
            assert!(!candidate.is_valid(), "expected {name} to be rejected");
        }
    }

    #[test]
    fn control_frames_are_bounded_and_typed() {
        assert_eq!(
            control_type(r#"{"type":"relay.peer_ready","generation":"a"}"#).as_deref(),
            Some("relay.peer_ready")
        );
        assert_eq!(
            control_type(r#"{"type":"relay.peer_unavailable"}"#).as_deref(),
            Some("relay.peer_unavailable")
        );
        assert_eq!(control_type("not json"), None);
        assert_eq!(control_type(r#"{"generation":"a"}"#), None);
        assert_eq!(control_type(r#"{"type":7}"#), None);
        assert_eq!(control_type(r#"["relay.peer_ready"]"#), None);
        let oversized = format!(
            r#"{{"type":"relay.peer_ready","pad":"{}"}}"#,
            "p".repeat(600)
        );
        assert_eq!(control_type(&oversized), None);
    }

    #[test]
    fn identifiers_and_fingerprints_must_be_exact() {
        assert_eq!(pair_id_bytes(&"b".repeat(32)).unwrap(), [0xbb; 16]);
        assert!(pair_id_bytes(&"b".repeat(30)).is_err());
        assert!(pair_id_bytes(&"B".repeat(32)).is_err());
        assert!(pair_id_bytes(&"z".repeat(32)).is_err());
        assert_eq!(fingerprint_bytes(&"c".repeat(64)).unwrap(), [0xcc; 32]);
        assert!(fingerprint_bytes(&"c".repeat(62)).is_err());
        assert!(fingerprint_bytes(&"g".repeat(64)).is_err());
    }
}
