use anyhow::Result;
use base64::engine::general_purpose::{
    STANDARD as B64, STANDARD_NO_PAD as B64_PAD_FREE, URL_SAFE_NO_PAD as B64URL,
};
use base64::Engine as _;
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
    // The JSON is what a person pastes by hand, so it stays readable. What goes
    // into the QR does not have to be: as JSON in a percent-encoded query it ran
    // to 963 characters, which is a 117-module symbol, and at the size a monitor
    // actually shows it that is under three pixels a module -- too fine for a
    // phone camera. The compact form carries the same fields in binary and fits
    // in roughly 73 modules, which scans from across a desk.
    let deep_link = match compact_link(payload) {
        Some(link) => link,
        // Anything the compact form cannot represent still pairs, just densely.
        None => format!("focusbridge://pair?payload={}", percent_encode(&json)),
    };
    let code = QrCode::new(deep_link.as_bytes())?;
    // Rendering small and letting the UI scale it up loses the module edges a
    // scanner needs, so this is generated well above its display size.
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

/// The compact QR encoding, version 3.
///
/// Every field of [`QrPayload`] is carried, in the fixed order below, so the phone
/// rebuilds an identical pairing. The sizes are exact because the values are
/// already fixed-width: 256-bit keys arrive as 64 hex characters, the pair id as
/// 32, the capability and Noise keys as base64 of 32 bytes, and every LAN
/// candidate as `wss://<IPv4>:<port>`. Holding them as text costs two to four
/// times the bytes, and bytes are what decide whether the code can be scanned.
///
/// ```text
///   0      version, 3
///   1      flags; bit 0 set when the relay and Noise blocks are present
///   2..18  device id, the raw UUID
///  18..50  pairing key
///  50..82  certificate fingerprint
///  82      number of LAN candidates, then 4 bytes of IPv4 and 2 of port each
///          when bit 0 is set: relay URL length, the URL, then the account key
///          (32), pair id (16), capability (32), desktop key (32) and PSK (32)
/// ```
const COMPACT_VERSION: u8 = 3;
const COMPACT_FLAG_RELAY: u8 = 1;

fn compact_link(payload: &QrPayload) -> Option<String> {
    let bytes = encode_compact(payload)?;
    Some(format!("focusbridge://pair?c={}", B64URL.encode(bytes)))
}

/// Returns `None` for anything outside the shape above, so an unusual payload
/// falls back to the JSON link rather than pairing a phone with wrong details.
pub fn encode_compact(payload: &QrPayload) -> Option<Vec<u8>> {
    if !payload.mode.eq_ignore_ascii_case("local") {
        return None;
    }
    // These were only ever set by the retired cloud mode; the relay block replaced
    // them, and carrying both would give the phone two answers to one question.
    if payload.relay_url.is_some() || payload.device_pair_id.is_some() {
        return None;
    }
    let relay = match (&payload.relay, &payload.noise) {
        (Some(relay), Some(noise)) => Some((relay, noise)),
        (None, None) => None,
        // A half-populated pair is unusable; let the JSON path apply its own rule.
        _ => return None,
    };

    let mut out = vec![
        COMPACT_VERSION,
        if relay.is_some() {
            COMPACT_FLAG_RELAY
        } else {
            0
        },
    ];
    out.extend_from_slice(&uuid_bytes(&payload.device_id)?);
    out.extend_from_slice(&hex_bytes::<32>(&payload.pairing_key)?);
    out.extend_from_slice(&hex_bytes::<32>(&payload.cert_fingerprint)?);

    // The endpoint is always the first candidate, so it is not stored twice.
    let mut candidates: Vec<&String> = Vec::new();
    for candidate in std::iter::once(&payload.endpoint).chain(payload.endpoint_candidates.iter()) {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    out.push(u8::try_from(candidates.len()).ok()?);
    for candidate in candidates {
        let (ip, port) = socket_parts(candidate)?;
        out.extend_from_slice(&ip.octets());
        out.extend_from_slice(&port.to_be_bytes());
    }

    if let Some((relay, noise)) = relay {
        let url = relay.url.as_bytes();
        out.push(u8::try_from(url.len()).ok()?);
        out.extend_from_slice(url);
        out.extend_from_slice(&hex_bytes::<32>(&relay.account_key)?);
        out.extend_from_slice(&hex_bytes::<16>(&relay.pair_id)?);
        out.extend_from_slice(&b64_bytes::<32>(&relay.capability)?);
        out.extend_from_slice(&b64_bytes::<32>(&noise.desktop_key)?);
        out.extend_from_slice(&b64_bytes::<32>(&noise.psk)?);
    }
    Some(out)
}

/// The inverse, kept beside the encoder so the two cannot drift. The phone has its
/// own implementation; the round-trip test here is what pins the wire format that
/// both sides have to agree on.
pub fn decode_compact(bytes: &[u8]) -> Option<QrPayload> {
    let mut cursor = CompactCursor::new(bytes);
    if cursor.byte()? != COMPACT_VERSION {
        return None;
    }
    let flags = cursor.byte()?;
    let device_id = uuid_string(cursor.take(16)?);
    let pairing_key = hex_string(cursor.take(32)?);
    let cert_fingerprint = hex_string(cursor.take(32)?);

    let count = cursor.byte()? as usize;
    let mut candidates = Vec::with_capacity(count);
    for _ in 0..count {
        let raw = cursor.take(6)?;
        let ip = std::net::Ipv4Addr::new(raw[0], raw[1], raw[2], raw[3]);
        let port = u16::from_be_bytes([raw[4], raw[5]]);
        candidates.push(format!("wss://{ip}:{port}"));
    }
    let endpoint = candidates.first()?.clone();

    let (relay, noise) = if flags & COMPACT_FLAG_RELAY != 0 {
        let url_len = cursor.byte()? as usize;
        let url = String::from_utf8(cursor.take(url_len)?.to_vec()).ok()?;
        let account_key = hex_string(cursor.take(32)?);
        let pair_id = hex_string(cursor.take(16)?);
        let capability = B64URL.encode(cursor.take(32)?);
        let desktop_key = B64.encode(cursor.take(32)?);
        let psk = B64.encode(cursor.take(32)?);
        (
            Some(QrRelay {
                url,
                account_key,
                pair_id,
                capability,
            }),
            Some(QrNoise { desktop_key, psk }),
        )
    } else {
        (None, None)
    };
    // Trailing bytes mean this is not the payload it claims to be.
    if !cursor.finished() {
        return None;
    }
    Some(QrPayload {
        v: if relay.is_some() { 2 } else { 1 },
        mode: "local".into(),
        endpoint,
        endpoint_candidates: candidates,
        relay_url: None,
        device_pair_id: None,
        device_id,
        pairing_key,
        cert_fingerprint,
        relay,
        noise,
    })
}

struct CompactCursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> CompactCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    /// Every read is bounds-checked, so a truncated or padded code is rejected
    /// rather than read past its end.
    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let slice = self.bytes.get(self.at..self.at.checked_add(len)?)?;
        self.at += len;
        Some(slice)
    }

    fn byte(&mut self) -> Option<u8> {
        self.take(1).map(|slice| slice[0])
    }

    fn finished(&self) -> bool {
        self.at == self.bytes.len()
    }
}

fn socket_parts(candidate: &str) -> Option<(std::net::Ipv4Addr, u16)> {
    let rest = candidate.strip_prefix("wss://")?;
    let (host, port) = rest.rsplit_once(':')?;
    Some((host.parse().ok()?, port.parse().ok()?))
}

fn uuid_bytes(value: &str) -> Option<[u8; 16]> {
    let hex: String = value.chars().filter(|c| *c != '-').collect();
    hex_bytes::<16>(&hex)
}

fn uuid_string(bytes: &[u8]) -> String {
    let hex = hex_string(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn hex_bytes<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for (slot, pair) in out.iter_mut().zip(value.as_bytes().chunks(2)) {
        *slot = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(out)
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Accepts either base64 alphabet, padded or not: the capability comes from the
/// relay and the Noise keys are produced here, and they need not agree.
fn b64_bytes<const N: usize>(value: &str) -> Option<[u8; N]> {
    let trimmed = value.trim_end_matches('=');
    let decoded = B64URL
        .decode(trimmed)
        .or_else(|_| B64_PAD_FREE.decode(trimmed))
        .ok()?;
    decoded.try_into().ok()
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

    /// A realistic pairing: a routed address plus one other adapter, and a live
    /// relay. This is the payload the desktop actually shows.
    fn realistic() -> QrPayload {
        QrPayload {
            v: 2,
            mode: "local".into(),
            endpoint: "wss://192.168.4.23:9173".into(),
            endpoint_candidates: vec![
                "wss://192.168.4.23:9173".into(),
                "wss://172.20.16.1:9173".into(),
            ],
            relay_url: None,
            device_pair_id: None,
            device_id: "3f2b9c1a-4d5e-4a7b-8c9d-0e1f2a3b4c5d".into(),
            pairing_key: "a".repeat(64),
            cert_fingerprint: "b".repeat(64),
            relay: Some(QrRelay {
                url: "https://focusbridge-relay.focusbridge.workers.dev".into(),
                account_key: "c".repeat(64),
                pair_id: "d".repeat(32),
                capability: B64URL.encode([7u8; 32]),
            }),
            noise: Some(QrNoise {
                desktop_key: B64.encode([9u8; 32]),
                psk: B64.encode([11u8; 32]),
            }),
        }
    }

    #[test]
    fn the_compact_form_round_trips_every_field() {
        let original = realistic();

        let decoded = decode_compact(&encode_compact(&original).unwrap()).unwrap();

        assert_eq!(decoded.device_id, original.device_id);
        assert_eq!(decoded.pairing_key, original.pairing_key);
        assert_eq!(decoded.cert_fingerprint, original.cert_fingerprint);
        assert_eq!(decoded.endpoint, original.endpoint);
        assert_eq!(decoded.endpoint_candidates, original.endpoint_candidates);
        let relay = decoded.relay.unwrap();
        let expected = original.relay.unwrap();
        assert_eq!(relay.url, expected.url);
        assert_eq!(relay.account_key, expected.account_key);
        assert_eq!(relay.pair_id, expected.pair_id);
        assert_eq!(relay.capability, expected.capability);
        let noise = decoded.noise.unwrap();
        assert_eq!(
            noise.desktop_key,
            original.noise.as_ref().unwrap().desktop_key
        );
        assert_eq!(noise.psk, original.noise.unwrap().psk);
    }

    #[test]
    fn a_lan_only_pairing_round_trips_without_a_relay() {
        let mut original = realistic();
        original.v = 1;
        original.relay = None;
        original.noise = None;

        let decoded = decode_compact(&encode_compact(&original).unwrap()).unwrap();

        assert_eq!(decoded.v, 1);
        assert!(decoded.relay.is_none());
        assert!(decoded.noise.is_none());
        assert_eq!(decoded.endpoint_candidates, original.endpoint_candidates);
    }

    /// The size is the point of the whole encoding. A QR is only as scannable as
    /// its module count, and the JSON link needed 963 characters, which no phone
    /// camera could read at the size a monitor shows. If a field is ever added,
    /// this is the test that should stop it silently undoing that.
    #[test]
    fn the_scanned_link_stays_short_enough_to_read_from_a_distance() {
        let link = compact_link(&realistic()).unwrap();

        assert!(link.starts_with("focusbridge://pair?c="));
        // 425 bytes is the last QR that fits in 73 modules at this error level.
        assert!(
            link.len() <= 425,
            "the pairing link grew to {} characters, which needs a denser QR",
            link.len()
        );
    }

    #[test]
    fn a_truncated_or_padded_code_is_refused_rather_than_half_read() {
        let bytes = encode_compact(&realistic()).unwrap();

        assert!(decode_compact(&bytes[..bytes.len() - 1]).is_none());
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(decode_compact(&extra).is_none());
        assert!(decode_compact(&[]).is_none());
        // A future version must not be read with today's field layout.
        let mut newer = bytes;
        newer[0] = COMPACT_VERSION + 1;
        assert!(decode_compact(&newer).is_none());
    }

    #[test]
    fn a_payload_the_compact_form_cannot_carry_falls_back_to_the_json_link() {
        let mut cloud = realistic();
        cloud.mode = "cloud".into();
        assert!(compact_link(&cloud).is_none());

        let mut half = realistic();
        half.noise = None;
        assert!(compact_link(&half).is_none());

        // And the QR is still produced, just the older dense way.
        assert!(make_qr(&cloud, 10)
            .unwrap()
            .deep_link
            .starts_with("focusbridge://pair?payload="));
    }

    #[test]
    fn the_qr_carries_the_compact_link_while_the_pasted_payload_stays_json() {
        let out = make_qr(&realistic(), 10).unwrap();

        assert!(out.deep_link.starts_with("focusbridge://pair?c="));
        assert!(out.payload.starts_with('{'));
        assert!(out.payload.contains("\"pairingKey\""));
    }

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
