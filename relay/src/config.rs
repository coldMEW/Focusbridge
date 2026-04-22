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
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read config {}", path.display()))?;
        let cfg: Config = toml::from_str(&text).context("parse config toml")?;
        Ok(cfg)
    }
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
        "#;
        let cfg: Config = toml::from_str(text).unwrap();
        assert_eq!(cfg.bind, "0.0.0.0:8443");
        assert_eq!(cfg.ping_interval_secs, 30);
        assert_eq!(cfg.max_message_bytes, 65536);
    }
}
