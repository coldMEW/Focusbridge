pub use focusbridge_core::cert::*;

use sha2::Digest;
use std::path::{Path, PathBuf};

pub fn app_cert_dir(app_data_root: &Path) -> PathBuf {
    app_data_root.join("certs")
}

pub fn load_or_generate(app_data_root: &Path) -> anyhow::Result<GeneratedCert> {
    let dir = app_cert_dir(app_data_root);
    std::fs::create_dir_all(&dir)?;
    let cert_path = dir.join("desktop-cert.pem");
    let key_path = dir.join("desktop-key.pem");
    if cert_path.exists() && key_path.exists() {
        let cert_pem = std::fs::read_to_string(&cert_path)?;
        let key_pem = std::fs::read_to_string(&key_path)?;
        let der = pem_to_der(&cert_pem)?;
        let fingerprint_sha256_hex = hex::encode(sha2::Sha256::digest(&der));
        return Ok(GeneratedCert {
            cert_pem,
            key_pem,
            fingerprint_sha256_hex,
        });
    }
    let generated = generate_self_signed("focusbridge-desktop")?;
    std::fs::write(cert_path, &generated.cert_pem)?;
    std::fs::write(key_path, &generated.key_pem)?;
    Ok(generated)
}

fn pem_to_der(cert_pem: &str) -> anyhow::Result<Vec<u8>> {
    use anyhow::Context;
    use rustls_pemfile::certs;
    use std::io::BufReader;

    let mut reader = BufReader::new(cert_pem.as_bytes());
    let cert = certs(&mut reader)
        .next()
        .context("missing persisted certificate")??;
    Ok(cert.as_ref().to_vec())
}
