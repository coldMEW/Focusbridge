use anyhow::{Context, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::io::BufReader;

pub fn rustls_config_from_pem(cert_pem: &str, key_pem: &str) -> Result<rustls::ServerConfig> {
    let mut cert_reader = BufReader::new(cert_pem.as_bytes());
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .context("parse cert pem")?;
    let mut key_reader = BufReader::new(key_pem.as_bytes());
    let key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut key_reader)
        .context("parse key pem")?
        .context("no private key")?;
    let cfg = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("build tls config")?;
    Ok(cfg)
}
