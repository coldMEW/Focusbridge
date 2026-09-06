use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub bind: String,
    pub tls_cert_path: String,
    pub tls_key_path: String,
    #[serde(default)]
    pub allow_plaintext: bool,
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
        ensure!(
            cfg.auth_token_secret.as_deref().is_some_and(|secret| secret.trim().len() >= 32),
            "auth_token_secret (or FOCUSBRIDGE_AUTH_TOKEN_SECRET) must contain at least 32 non-padding bytes"
        );
        ensure!(
            cfg.ping_interval_secs > 0,
            "ping_interval_secs must be greater than zero"
        );
        ensure!(
            cfg.pairing_ttl_secs > 0,
            "pairing_ttl_secs must be greater than zero"
        );
        ensure!(
            cfg.max_message_bytes > 0,
            "max_message_bytes must be greater than zero"
        );
        ensure!(
            cfg.rate_limit_per_min > 0,
            "rate_limit_per_min must be greater than zero"
        );
        Ok(cfg)
    }

    /// Decide before binding; propagate TLS loading errors rather than falling back.
    pub fn should_use_tls(&self, cert_exists: bool, key_exists: bool) -> Result<bool> {
        ensure!(
            cert_exists == key_exists,
            "TLS certificate and key must both exist; partial TLS configuration is not allowed"
        );
        ensure!(
            cert_exists || self.allow_plaintext,
            "TLS certificate and key are missing; plaintext requires allow_plaintext = true"
        );
        Ok(cert_exists)
    }
}

fn default_auth_store_path() -> String {
    "data/auth-users.json".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_text(text: &str) -> Result<Config> {
        let path =
            std::env::temp_dir().join(format!("focusbridge-config-{}.toml", uuid::Uuid::new_v4()));
        std::fs::write(&path, text).unwrap();
        let result = Config::load(&path);
        std::fs::remove_file(path).unwrap();
        result
    }

    const VALID: &str = r#"
bind = "127.0.0.1:8443"
tls_cert_path = "certs/server.crt"
tls_key_path = "certs/server.key"
ping_interval_secs = 30
pairing_ttl_secs = 300
max_message_bytes = 65536
rate_limit_per_min = 120
auth_token_secret = "0123456789abcdef0123456789abcdef"
"#;

    #[test]
    fn load_rejects_weak_secret() {
        for secret in [
            "",
            "short",
            "                               ",
            "0123456789abcdef0123456789abcde",
        ] {
            let text = VALID.replace("0123456789abcdef0123456789abcdef", secret);
            assert!(load_text(&text).is_err(), "accepted weak secret");
        }
    }

    #[test]
    fn load_rejects_zero_settings() {
        for setting in [
            "ping_interval_secs = 30",
            "pairing_ttl_secs = 300",
            "max_message_bytes = 65536",
            "rate_limit_per_min = 120",
        ] {
            let name = setting.split_once(" = ").unwrap().0;
            assert!(
                load_text(&VALID.replace(setting, &format!("{name} = 0"))).is_err(),
                "accepted zero {name}"
            );
        }
    }

    #[test]
    fn load_accepts_valid_settings() {
        assert!(load_text(VALID).is_ok());
    }

    #[test]
    fn load_resolves_secret_from_environment() {
        const CHILD: &str = "FOCUSBRIDGE_CONFIG_TEST_CHILD";
        if let Ok(case) = std::env::var(CHILD) {
            let text = VALID.replace(
                "auth_token_secret = \"0123456789abcdef0123456789abcdef\"",
                "",
            );
            assert_eq!(load_text(&text).is_ok(), case == "valid");
            return;
        }
        // Isolate environment changes from other tests and the developer's shell.
        for (case, secret) in [
            ("missing", None),
            ("weak", Some("short")),
            ("valid", Some("0123456789abcdef0123456789abcdef")),
        ] {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "config::tests::load_resolves_secret_from_environment",
                ])
                .env(CHILD, case)
                .env_remove("FOCUSBRIDGE_AUTH_TOKEN_SECRET");
            if let Some(secret) = secret {
                command.env("FOCUSBRIDGE_AUTH_TOKEN_SECRET", secret);
            }
            assert!(
                command.status().unwrap().success(),
                "environment case {case}"
            );
        }
    }

    #[test]
    fn tls_selection_fails_closed() {
        let mut cfg = load_text(VALID).unwrap();
        assert!(!cfg.allow_plaintext);
        for allow in [false, true] {
            cfg.allow_plaintext = allow;
            assert!(cfg.should_use_tls(true, true).unwrap());
            assert!(cfg.should_use_tls(true, false).is_err());
            assert!(cfg.should_use_tls(false, true).is_err());
            if allow {
                assert!(!cfg.should_use_tls(false, false).unwrap());
            } else {
                assert!(cfg.should_use_tls(false, false).is_err());
            }
        }
        assert!(
            load_text(&format!("{VALID}\nallow_plaintext = true"))
                .unwrap()
                .allow_plaintext
        );
    }

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
