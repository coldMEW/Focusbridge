//! Device-only key material for the cross-network relay sessions.
//!
//! Everything here is stored in the SQLCipher-encrypted settings table, whose key
//! is wrapped by Windows DPAPI (or the macOS Keychain). None of it is ever sent to
//! the relay: the relay only ever sees routing metadata and an opaque capability.

use crate::db::store;
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use focusbridge_secure_channel::Identity;
use rand::{rngs::OsRng, RngCore};
use std::path::Path;
use zeroize::Zeroizing;

const IDENTITY_KEY: &str = "relay.identity.private.v1";
const PAIR_PREFIX: &str = "relay.pair.";

/// Loads this desktop's long-lived Noise static key, creating it on first use.
///
/// The key is never rotated automatically: phones pin the matching public key at
/// enrollment, so replacing it would present this PC as an unknown device rather
/// than recovering the pairing.
pub fn identity(db_path: &Path) -> Result<Identity> {
    if let Some(stored) = store::get_setting(db_path, IDENTITY_KEY)? {
        let decoded = Zeroizing::new(
            B64.decode(stored.trim())
                .context("decode stored desktop identity key")?,
        );
        let bytes: [u8; 32] = decoded
            .as_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("stored desktop identity key has the wrong length"))?;
        return Identity::from_private(bytes).context("load desktop identity key");
    }

    let identity = Identity::generate().context("generate desktop identity key")?;
    let private = identity.export_private();
    store::set_setting(db_path, IDENTITY_KEY, &B64.encode(private.as_slice()))
        .context("persist desktop identity key")?;
    // Prove the record round-trips before any phone pins the public half.
    let reloaded = store::get_setting(db_path, IDENTITY_KEY)?
        .context("desktop identity key was not persisted")?;
    if B64.decode(reloaded.trim()).ok().as_deref() != Some(private.as_slice()) {
        bail!("desktop identity key did not persist correctly");
    }
    Ok(identity)
}

/// The per-pairing secrets that never leave the two devices.
#[derive(Clone)]
pub struct PairSecrets {
    /// Enrollment pre-shared key, also used as the Noise `psk3` for every session.
    pub psk: [u8; 32],
    /// The phone's static public key, present once a phone has been approved.
    pub phone: Option<[u8; 32]>,
}

fn pair_setting(pair_id: &str) -> String {
    format!("{PAIR_PREFIX}{pair_id}")
}

/// Creates and stores a fresh pre-shared key for a newly provisioned relay pair.
pub fn create_pair_secrets(db_path: &Path, pair_id: &str) -> Result<[u8; 32]> {
    let mut psk = [0u8; 32];
    OsRng.fill_bytes(&mut psk);
    store::set_setting(db_path, &pair_setting(pair_id), &B64.encode(psk))
        .context("persist relay pair secret")?;
    Ok(psk)
}

pub fn pair_secrets(db_path: &Path, pair_id: &str) -> Result<Option<PairSecrets>> {
    let Some(stored) = store::get_setting(db_path, &pair_setting(pair_id))?
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    // "<base64 psk>" before a phone is approved, "<base64 psk>:<base64 phone key>" after.
    let (psk, phone) = match stored.split_once(':') {
        Some((psk, phone)) => (psk, Some(phone)),
        None => (stored.as_str(), None),
    };
    let psk: [u8; 32] = B64
        .decode(psk.trim())
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .context("stored relay pair secret is unreadable")?;
    let phone = match phone {
        Some(value) => Some(
            B64.decode(value.trim())
                .ok()
                .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
                .context("stored phone identity is unreadable")?,
        ),
        None => None,
    };
    Ok(Some(PairSecrets { psk, phone }))
}

/// Pins the phone that completed enrollment. Refuses to overwrite a different
/// phone: a second device must be enrolled through its own pair, so a stolen or
/// replayed QR cannot silently take over an existing pairing.
pub fn approve_phone(db_path: &Path, pair_id: &str, phone: [u8; 32]) -> Result<()> {
    let secrets = pair_secrets(db_path, pair_id)?.context("unknown relay pair")?;
    match secrets.phone {
        Some(existing) if existing != phone => {
            bail!("another phone is already paired to this relay pair")
        }
        Some(_) => return Ok(()),
        None => {}
    }
    store::set_setting(
        db_path,
        &pair_setting(pair_id),
        &format!("{}:{}", B64.encode(secrets.psk), B64.encode(phone)),
    )
    .context("persist approved phone identity")
}

pub fn forget_pair(db_path: &Path, pair_id: &str) -> Result<()> {
    store::set_setting(db_path, &pair_setting(pair_id), "").context("clear relay pair secret")
}
