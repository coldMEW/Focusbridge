use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub bind: String,
    pub tls_cert_path: String,
    pub tls_key_path: String,
    pub ping_interval_secs: u64,
    pub pairing_ttl_secs: u64,
    pub max_message_bytes: usize,
    pub rate_limit_per_min: u32,
    #[serde(default = "default_auth_store_path")]
    pub auth_store_path: String,
    #[serde(default)]
    pub auth_token_secret: Option<String>,
    #[serde(default)]
    pub google_client_id: Option<String>,
    #[serde(default)]
    pub resend_api_key: Option<String>,
    #[serde(default)]
    pub otp_email_from: Option<String>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read config {}", path.display()))?;
        let mut cfg: Config = toml::from_str(&text).context("parse config toml")?;
        cfg.auth_token_secret = cfg.auth_token_secret.or_else(|| {
            std::env::var("FOCUSBRIDGE_AUTH_TOKEN_SECRET")
                .ok()
                .filter(|v| !v.trim().is_empty())
        });
        cfg.google_client_id = cfg.google_client_id.or_else(|| {
            std::env::var("FOCUSBRIDGE_GOOGLE_CLIENT_ID")
                .ok()
                .filter(|v| !v.trim().is_empty())
        });
        cfg.resend_api_key = cfg.resend_api_key.or_else(|| {
            std::env::var("FOCUSBRIDGE_RESEND_API_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty())
        });
        cfg.otp_email_from = cfg.otp_email_from.or_else(|| {
            std::env::var("FOCUSBRIDGE_OTP_EMAIL_FROM")
                .ok()
                .filter(|v| !v.trim().is_empty())
        });
        Ok(cfg)
    }
}

fn default_auth_store_path() -> String {
    "data/auth-users.json".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_shape() {
        let text = r#"
bind = "0.0.0.0:8443"
tls_cert_path = "certs/server.crt"
tls_key_path  = "certs/server.key"
ping_interval_secs = 30
pairing_ttl_secs = 300
max_message_bytes = 65536
rate_limit_per_min = 120
auth_store_path = "data/auth-users.json"
        "#;
        let cfg: Config = toml::from_str(text).unwrap();
        assert_eq!(cfg.bind, "0.0.0.0:8443");
        assert_eq!(cfg.ping_interval_secs, 30);
        assert_eq!(cfg.max_message_bytes, 65536);
        assert_eq!(cfg.auth_store_path, "data/auth-users.json");
    }
}
