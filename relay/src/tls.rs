use anyhow::{Context, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub fn load_tls_config(cert_path: &Path, key_path: &Path) -> Result<rustls::ServerConfig> {
    let certs = load_certs(cert_path)?;
    let key = load_key(key_path)?;
    let cfg = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("build rustls server config")?;
    Ok(cfg)
}

fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    let f = File::open(path).with_context(|| format!("open cert {}", path.display()))?;
    let mut r = BufReader::new(f);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut r)
        .collect::<Result<Vec<_>, _>>()
        .context("read pem certs")?;
    if certs.is_empty() {
        anyhow::bail!("no certs found in {}", path.display());
    }
    Ok(certs)
}

fn load_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    let f = File::open(path).with_context(|| format!("open key {}", path.display()))?;
    let mut r = BufReader::new(f);
    let key = rustls_pemfile::private_key(&mut r)
        .context("parse private key")?
        .context("no private key in file")?;
    Ok(key)
}
