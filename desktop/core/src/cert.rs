use anyhow::Result;
use rcgen::{CertificateParams, DnType, KeyPair, SanType};
use sha2::{Digest, Sha256};

pub struct GeneratedCert {
    pub cert_pem: String,
    pub key_pem: String,
    pub fingerprint_sha256_hex: String,
}

pub fn generate_self_signed(cn: &str) -> Result<GeneratedCert> {
    let mut params = CertificateParams::new(vec![cn.to_string()]);
    params.distinguished_name.push(DnType::CommonName, cn);
    params
        .subject_alt_names
        .push(SanType::DnsName("localhost".into()));
    let key = KeyPair::generate(&rcgen::PKCS_ECDSA_P256_SHA256)?;
    params.key_pair = Some(key);
    let cert = rcgen::Certificate::from_params(params)?;
    let cert_pem = cert.serialize_pem()?;
    let key_pem = cert.serialize_private_key_pem();
    let der = cert.serialize_der()?;
    let fingerprint_sha256_hex = hex::encode(Sha256::digest(&der));
    Ok(GeneratedCert {
        cert_pem,
        key_pem,
        fingerprint_sha256_hex,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cert_has_expected_shape() {
        let g = generate_self_signed("focusbridge-desktop").unwrap();
        assert!(g.cert_pem.contains("BEGIN CERTIFICATE"));
        assert_eq!(g.fingerprint_sha256_hex.len(), 64);
    }
}
