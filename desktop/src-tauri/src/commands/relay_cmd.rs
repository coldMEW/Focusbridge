//! Commands for the opt-in cross-network relay.
//!
//! Enabling the relay is an account action: it provisions one revocable pair at
//! the relay and a fresh device-only pre-shared key held on this PC. LAN pairing
//! keeps working with no account and no relay, so nothing here is on the critical
//! path for same-network sync.

use crate::state::AppState;
use crate::sync::{relay_api, relay_identity};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayStatus {
    /// True when a pair exists and this desktop can dial the relay.
    pub configured: bool,
    pub relay_url: String,
    pub pair_id: Option<String>,
    pub expires_at: Option<i64>,
    /// True once a phone has completed enrollment and had its identity pinned.
    pub phone_enrolled: bool,
}

fn status(state: &AppState) -> Result<RelayStatus, String> {
    let relay_url = relay_api::relay_url(&state.db_path).map_err(|error| error.to_string())?;
    let pair = relay_api::current_pair(&state.db_path).map_err(|error| error.to_string())?;
    let phone_enrolled = match pair.as_ref() {
        Some(pair) => relay_identity::pair_secrets(&state.db_path, &pair.pair_id)
            .map_err(|error| error.to_string())?
            .is_some_and(|secrets| secrets.phone.is_some()),
        None => false,
    };
    Ok(RelayStatus {
        configured: pair.is_some(),
        relay_url,
        pair_id: pair.as_ref().map(|pair| pair.pair_id.clone()),
        expires_at: pair.as_ref().map(|pair| pair.expires_at),
        phone_enrolled,
    })
}

#[tauri::command]
pub fn relay_status(state: tauri::State<'_, AppState>) -> Result<RelayStatus, String> {
    status(&state)
}

/// Points this desktop at a different relay deployment. Existing pairs are
/// cleared because their capabilities are only valid at the relay that issued them.
#[tauri::command]
pub fn relay_set_url(
    url: String,
    state: tauri::State<'_, AppState>,
) -> Result<RelayStatus, String> {
    relay_api::set_relay_url(&state.db_path, &url).map_err(|error| error.to_string())?;
    relay_api::clear_pair(&state.db_path).map_err(|error| error.to_string())?;
    status(&state)
}

/// Provisions a relay pair for the signed-in account and mints this pairing's
/// device-only pre-shared key.
///
/// `id_token` is a Firebase ID token obtained by the UI for this request only. It
/// is never written to disk and never reaches the phone.
#[tauri::command]
pub async fn relay_enable(
    id_token: String,
    state: tauri::State<'_, AppState>,
) -> Result<RelayStatus, String> {
    let db_path = state.db_path.clone();
    let pair = relay_api::provision(&db_path, &id_token)
        .await
        .map_err(|error| error.to_string())?;
    // A brand new pair has no enrolled phone, so it always gets a brand new key.
    relay_identity::create_pair_secrets(&db_path, &pair.pair_id)
        .map_err(|error| error.to_string())?;
    // Fail now rather than at the first connection if identity storage is broken.
    relay_identity::identity(&db_path).map_err(|error| error.to_string())?;
    status(&state)
}

/// Revokes the pair at the relay and forgets its device-only key material, so
/// neither routing nor a stored session can be resumed.
#[tauri::command]
pub async fn relay_disable(
    id_token: String,
    state: tauri::State<'_, AppState>,
) -> Result<RelayStatus, String> {
    let db_path = state.db_path.clone();
    let pair = relay_api::current_pair(&db_path).map_err(|error| error.to_string())?;
    if let Some(pair) = pair.as_ref() {
        // Revoke first: local state must not claim the relay is disabled while the
        // capability would still route.
        relay_api::revoke(&db_path, &id_token, &pair.pair_id)
            .await
            .map_err(|error| error.to_string())?;
        relay_identity::forget_pair(&db_path, &pair.pair_id).map_err(|error| error.to_string())?;
    }
    relay_api::clear_pair(&db_path).map_err(|error| error.to_string())?;
    status(&state)
}
