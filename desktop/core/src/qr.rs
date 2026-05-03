use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use image::{ImageBuffer, Luma};
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrPayload {
    pub v: u32,
    pub mode: String,
    pub endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay_url: Option<String>,
    pub device_id: String,
    pub pairing_key: String,
    pub cert_fingerprint: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrOutput {
    pub payload: String,
    pub png_base64: String,
    pub expires_at: i64,
}

pub fn make_qr(payload: &QrPayload, expires_at: i64) -> Result<QrOutput> {
    let json = serde_json::to_string(payload)?;
    let code = QrCode::new(json.as_bytes())?;
    let img = code.render::<Luma<u8>>().min_dimensions(256, 256).build();
    let (w, h) = (img.width(), img.height());
    let buf: ImageBuffer<Luma<u8>, Vec<u8>> = ImageBuffer::from_raw(w, h, img.into_raw()).unwrap();
    let mut png_bytes: Vec<u8> = Vec::new();
    buf.write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)?;
    Ok(QrOutput {
        payload: json,
        png_base64: B64.encode(&png_bytes),
        expires_at,
    })
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
            relay_url: None,
            device_id: "id".into(),
            pairing_key: "a".repeat(64),
            cert_fingerprint: "b".repeat(64),
        };
        let out = make_qr(&p, 10).unwrap();
        assert!(!out.png_base64.is_empty());
        assert_eq!(out.expires_at, 10);
    }
}
