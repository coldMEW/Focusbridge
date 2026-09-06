//! Owner-authenticated provisioning against the FocusBridge relay Worker.
//!
//! The relay issues one routing capability per role. Capabilities authorize
//! message routing only; they cannot read application traffic, which is protected
//! by the device-only Noise session. The phone's capability travels in the QR, the
//! desktop's never leaves this machine.

use crate::db::store;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

pub const DEFAULT_RELAY_URL: &str = "https://focusbridge-relay.focusbridge.workers.dev";

const URL_SETTING: &str = "relay.url";
const PAIR_SETTING: &str = "relay.current.pair";

pub use focusbridge_core::relay::RelayPair;

#[derive(Deserialize)]
struct Capabilities {
    desktop: String,
    phone: String,
}

#[derive(Deserialize)]
struct CreatedPair {
    #[serde(rename = "accountKey")]
    account_key: String,
    #[serde(rename = "pairId")]
    pair_id: String,
    #[serde(rename = "expiresAt")]
    expires_at: i64,
    capabilities: Capabilities,
}

const AUTO_CONNECT_SETTING: &str = "relay.auto_connect";

/// Whether this PC rejoins the last paired phone on its own.
///
/// Off, the relay stays idle until the user picks a phone from previous
/// connections or shows a new QR. That matters when several phones have been
/// paired: reattaching to whichever one answers first is not always wanted.
/// Defaults to on so an upgrade does not quietly stop syncing.
pub fn auto_connect(db_path: &Path) -> Result<bool> {
    Ok(store::get_setting(db_path, AUTO_CONNECT_SETTING)?.as_deref() != Some("false"))
}

pub fn set_auto_connect(db_path: &Path, enabled: bool) -> Result<()> {
    store::set_setting(
        db_path,
        AUTO_CONNECT_SETTING,
        if enabled { "true" } else { "false" },
    )
}

pub fn relay_url(db_path: &Path) -> Result<String> {
    Ok(store::get_setting(db_path, URL_SETTING)?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_RELAY_URL.to_string()))
}

pub fn set_relay_url(db_path: &Path, url: &str) -> Result<()> {
    let url = url.trim().trim_end_matches('/');
    ensure!(
        url.starts_with("https://") && url.len() > 8 && !url.contains(char::is_whitespace),
        "the relay endpoint must be an https URL"
    );
    store::set_setting(db_path, URL_SETTING, url)
}

/// Requests a new pair from the relay using a verified Firebase ID token.
///
/// The token is used exactly once, for this request, and is never stored.
pub async fn provision(db_path: &Path, id_token: &str) -> Result<RelayPair> {
    let token = id_token.trim();
    ensure!(
        !token.is_empty()
            && token.len() <= 8192
            && token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_')),
        "a valid signed-in account is required to enable cross-network sync"
    );
    let url = relay_url(db_path)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .https_only(true)
        .build()
        .context("build relay client")?;
    let response = client
        .post(format!("{url}/v1/pairs"))
        .bearer_auth(token)
        .header("Content-Length", "0")
        .send()
        .await
        .context("reach the FocusBridge relay")?;

    let status = response.status();
    // The body may carry a relay error code, never a credential; a failed request
    // must not leave a half-written pair behind.
    let body = response.text().await.unwrap_or_default();
    ensure!(
        status.is_success(),
        "{}",
        match status.as_u16() {
            401 => "the relay rejected this account; sign in again and retry".to_string(),
            409 => "this account already has the maximum number of paired devices".to_string(),
            429 => "too many pairing attempts on this account; try again later".to_string(),
            other => format!("the relay refused to create a pair (HTTP {other})"),
        }
    );
    let created: CreatedPair = serde_json::from_str(&body).context("read the relay response")?;
    let pair = RelayPair {
        url,
        account_key: created.account_key,
        pair_id: created.pair_id,
        desktop_capability: created.capabilities.desktop,
        phone_capability: created.capabilities.phone,
        expires_at: created.expires_at,
    };
    ensure!(pair.is_valid(), "the relay returned a malformed pair");
    save_pair(db_path, &pair)?;
    Ok(pair)
}

/// Revokes a pair at the relay so its capabilities stop routing immediately.
pub async fn revoke(db_path: &Path, id_token: &str, pair_id: &str) -> Result<()> {
    let url = relay_url(db_path)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .https_only(true)
        .build()
        .context("build relay client")?;
    let response = client
        .delete(format!("{url}/v1/pairs/{pair_id}"))
        .bearer_auth(id_token.trim())
        .send()
        .await
        .context("reach the FocusBridge relay")?;
    // A pair the relay has already forgotten is the state we wanted.
    ensure!(
        response.status().is_success() || response.status().as_u16() == 404,
        "the relay refused to revoke this pair (HTTP {})",
        response.status().as_u16()
    );
    Ok(())
}

pub fn save_pair(db_path: &Path, pair: &RelayPair) -> Result<()> {
    ensure!(pair.is_valid(), "refusing to store a malformed relay pair");
    store::set_setting(
        db_path,
        PAIR_SETTING,
        &serde_json::json!({
            "url": pair.url,
            "accountKey": pair.account_key,
            "pairId": pair.pair_id,
            "desktopCapability": pair.desktop_capability,
            "phoneCapability": pair.phone_capability,
            "expiresAt": pair.expires_at,
        })
        .to_string(),
    )
}

pub fn current_pair(db_path: &Path) -> Result<Option<RelayPair>> {
    let Some(stored) =
        store::get_setting(db_path, PAIR_SETTING)?.filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let value: serde_json::Value = match serde_json::from_str(&stored) {
        Ok(value) => value,
        // A corrupt record must not block LAN pairing; it is simply not usable.
        Err(_) => return Ok(None),
    };
    let text = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let pair = RelayPair {
        url: text("url"),
        account_key: text("accountKey"),
        pair_id: text("pairId"),
        desktop_capability: text("desktopCapability"),
        phone_capability: text("phoneCapability"),
        expires_at: value
            .get("expiresAt")
            .and_then(|v| v.as_i64())
            .unwrap_or_default(),
    };
    if !pair.is_valid() {
        return Ok(None);
    }
    Ok(Some(pair))
}

pub fn clear_pair(db_path: &Path) -> Result<()> {
    store::set_setting(db_path, PAIR_SETTING, "")
}

/// `wss://host/v1/socket/{accountKey}/{pairId}/desktop` for this pair.
pub fn socket_url(pair: &RelayPair) -> String {
    pair.socket_url("desktop")
}
