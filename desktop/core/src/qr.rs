use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use image::{ImageBuffer, Luma};
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

/// Cross-network rendezvous, added in QR version 2. `url`, `account_key` and
/// `pair_id` are routing metadata; `capability` is the phone's revocable transport
/// token. None of them can read application traffic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QrRelay {
    pub url: String,
    pub account_key: String,
    pub pair_id: String,
    pub capability: String,
}

/// Device-only key material: the desktop static public key the phone pins, and the
/// single-pairing enrollment pre-shared key. Neither is ever sent to the relay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QrNoise {
    pub desktop_key: String,
    pub psk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrPayload {
    pub v: u32,
    pub mode: String,
    pub endpoint: String,
    #[serde(
        rename = "endpointCandidates",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub endpoint_candidates: Vec<String>,
    #[serde(rename = "relayUrl", skip_serializing_if = "Option::is_none")]
    pub relay_url: Option<String>,
    #[serde(rename = "devicePairId", skip_serializing_if = "Option::is_none")]
    pub device_pair_id: Option<String>,
    #[serde(rename = "deviceId")]
    pub device_id: String,
    #[serde(rename = "pairingKey")]
    pub pairing_key: String,
    #[serde(rename = "certFingerprint")]
    pub cert_fingerprint: String,
    /// Present together or not at all: a half-populated pair is unusable and the
    /// phone must fall back to LAN rather than dial a relay it cannot authenticate to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay: Option<QrRelay>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noise: Option<QrNoise>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrOutput {
    pub payload: String,
    pub deep_link: String,
    pub png_base64: String,
    pub expires_at: i64,
}

pub fn make_qr(payload: &QrPayload, expires_at: i64) -> Result<QrOutput> {
    let json = serde_json::to_string(payload)?;
    let deep_link = format!("focusbridge://pair?payload={}", percent_encode(&json));
    let code = QrCode::new(deep_link.as_bytes())?;
    // The version 2 payload needs roughly 117 modules a side. Rendering small and
    // letting the UI scale it up loses the module edges a scanner needs, so this
    // is generated well above its display size.
    let img = code.render::<Luma<u8>>().min_dimensions(768, 768).build();
    let (w, h) = (img.width(), img.height());
    let buf: ImageBuffer<Luma<u8>, Vec<u8>> = ImageBuffer::from_raw(w, h, img.into_raw()).unwrap();
    let mut png_bytes: Vec<u8> = Vec::new();
    buf.write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)?;
    Ok(QrOutput {
        payload: json,
        deep_link,
        png_base64: B64.encode(&png_bytes),
        expires_at,
    })
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_png() {
        let p = QrPayload {
            v: 1,
            mode: "local".into(),
            endpoint: "1.2.3.4:9173".into(),
            endpoint_candidates: vec![],
            relay_url: None,
            device_pair_id: None,
            device_id: "id".into(),
            pairing_key: "a".repeat(64),
            cert_fingerprint: "b".repeat(64),
            relay: None,
            noise: None,
        };
        let out = make_qr(&p, 10).unwrap();
        assert!(!out.png_base64.is_empty());
        assert_eq!(out.expires_at, 10);
    }

    #[test]
    fn qr_code_uses_focusbridge_deep_link_while_manual_payload_stays_json() {
        let p = QrPayload {
            v: 1,
            mode: "local".into(),
            endpoint: "1.2.3.4:9173".into(),
            endpoint_candidates: vec![],
            relay_url: None,
            device_pair_id: None,
            device_id: "id".into(),
            pairing_key: "a".repeat(64),
            cert_fingerprint: "b".repeat(64),
            relay: None,
            noise: None,
        };

        let out = make_qr(&p, 10).unwrap();

        assert!(out.payload.starts_with('{'));
        assert!(out.deep_link.starts_with("focusbridge://pair?payload="));
    }

    #[test]
    fn version_two_payloads_carry_relay_and_noise_blocks() {
        let p = QrPayload {
            v: 2,
            mode: "local".into(),
            endpoint: "wss://1.2.3.4:9173".into(),
            endpoint_candidates: vec!["wss://1.2.3.4:9173".into()],
            relay_url: None,
            device_pair_id: None,
            device_id: "id".into(),
            pairing_key: "a".repeat(64),
            cert_fingerprint: "b".repeat(64),
            relay: Some(QrRelay {
                url: "https://relay.example".into(),
                account_key: "c".repeat(64),
                pair_id: "d".repeat(32),
                capability: "E".repeat(43),
            }),
            noise: Some(QrNoise {
                desktop_key: "AAAA".into(),
                psk: "BBBB".into(),
            }),
        };

        let payload = make_qr(&p, 10).unwrap().payload;

        // Field names must match the Android serializer exactly.
        assert!(payload.contains("\"relay\":{\"url\":\"https://relay.example\""));
        assert!(payload.contains("\"accountKey\""));
        assert!(payload.contains("\"pairId\""));
        assert!(payload.contains("\"capability\""));
        assert!(payload.contains("\"noise\":{\"desktopKey\":\"AAAA\",\"psk\":\"BBBB\"}"));
        assert!(!payload.contains("account_key"));
        assert!(!payload.contains("desktop_key"));
    }

    #[test]
    fn payloads_without_a_relay_omit_both_blocks_entirely() {
        let p = QrPayload {
            v: 1,
            mode: "local".into(),
            endpoint: "wss://1.2.3.4:9173".into(),
            endpoint_candidates: vec![],
            relay_url: None,
            device_pair_id: None,
            device_id: "id".into(),
            pairing_key: "a".repeat(64),
            cert_fingerprint: "b".repeat(64),
            relay: None,
            noise: None,
        };

        let payload = make_qr(&p, 10).unwrap().payload;

        assert!(!payload.contains("relay"));
        assert!(!payload.contains("noise"));
    }

    #[test]
    fn serializes_android_compatible_camel_case_fields() {
        let p = QrPayload {
            v: 1,
            mode: "cloud".into(),
            endpoint: "1.2.3.4:9173".into(),
            endpoint_candidates: vec!["1.2.3.4:9173".into(), "10.0.0.5:9173".into()],
            relay_url: Some("https://relay.example".into()),
            device_pair_id: Some("pair_123".into()),
            device_id: "id".into(),
            pairing_key: "a".repeat(64),
            cert_fingerprint: "b".repeat(64),
            relay: None,
            noise: None,
        };

        let payload = make_qr(&p, 10).unwrap().payload;

        assert!(payload.contains("\"relayUrl\""));
        assert!(payload.contains("\"devicePairId\""));
        assert!(payload.contains("\"deviceId\""));
        assert!(payload.contains("\"pairingKey\""));
        assert!(payload.contains("\"certFingerprint\""));
        assert!(payload.contains("\"endpointCandidates\""));
        assert!(!payload.contains("relay_url"));
    }
}
