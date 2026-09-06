//! Database keys are independent of UI authentication. Windows persists only a
//! user-scoped DPAPI envelope; macOS stores the key in the user's Keychain.

use anyhow::{bail, ensure, Context, Result};
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

pub struct DatabaseKey(Zeroizing<[u8; 32]>);

impl DatabaseKey {
    pub(crate) fn sqlcipher_key(&self) -> Zeroizing<String> {
        let mut text = Zeroizing::new(String::with_capacity(67));
        text.push_str("x'");
        // Do not allocate an additional, non-zeroizing hex copy of the key.
        for byte in self.0.iter() {
            use std::fmt::Write;
            write!(&mut *text, "{byte:02x}").expect("write to key buffer");
        }
        text.push('\'');
        text
    }

    #[cfg(test)]
    pub(crate) fn for_test(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }
}

pub fn key_path(database: &Path) -> PathBuf {
    let mut path = database.as_os_str().to_os_string();
    path.push(".key.dpapi");
    path.into()
}

pub(crate) fn load_or_create(database: &Path, allow_create: bool) -> Result<DatabaseKey> {
    #[cfg(windows)]
    {
        windows::load_or_create(&key_path(database), allow_create)
    }
    #[cfg(target_os = "macos")]
    {
        macos::load_or_create(database, allow_create)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (database, allow_create);
        bail!("encrypted database storage requires Windows DPAPI or macOS Keychain; native key storage is not implemented on this platform (no plaintext fallback)")
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use rand::{rngs::OsRng, RngCore};
    use security_framework::os::macos::{keychain::SecKeychain, passwords::find_generic_password};
    use sha2::{Digest, Sha256};
    use std::os::unix::ffi::OsStrExt;

    const SERVICE: &str = "com.focusbridge.desktop.sqlcipher.v1";
    const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

    pub(super) fn load_or_create(database: &Path, allow_create: bool) -> Result<DatabaseKey> {
        let path = database
            .parent()
            .context("database parent")?
            .canonicalize()?
            .join(database.file_name().context("database filename")?);
        let account = hex::encode(Sha256::digest(path.as_os_str().as_bytes()));
        let keychain = SecKeychain::default().context("open user Keychain for database key")?;
        let key = load(&keychain, &account, allow_create)?;
        if !database.try_exists()? {
            let recovery_exists = [".encrypting", ".plaintext-migration"].iter().try_fold(
                false,
                |found, suffix| -> Result<bool> {
                    let mut sibling = database.as_os_str().to_os_string();
                    sibling.push(suffix);
                    Ok(found || PathBuf::from(sibling).try_exists()?)
                },
            )?;
            ensure!(
                key.1 || recovery_exists,
                "database is missing but its Keychain key exists; refusing an empty replacement"
            );
        }
        Ok(key.0)
    }

    fn load(
        keychain: &SecKeychain,
        account: &str,
        allow_create: bool,
    ) -> Result<(DatabaseKey, bool)> {
        match find_generic_password(Some(std::slice::from_ref(keychain)), SERVICE, account) {
            Ok((password, _)) => {
                ensure!(password.len() == 32, "invalid Keychain database key; key was not reset");
                let mut key = Zeroizing::new([0u8; 32]);
                key.copy_from_slice(password.as_ref());
                Ok((DatabaseKey(key), false))
            }
            Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND && allow_create => {
                let mut key = Zeroizing::new([0u8; 32]);
                OsRng.try_fill_bytes(&mut *key).context("generate database key from OS entropy")?;
                // Add, never update: a concurrent/existing Keychain item is an error.
                keychain.add_generic_password(SERVICE, account, &*key)
                    .context("create database key in user Keychain without replacement")?;
                let persisted = load(keychain, account, false)?.0;
                ensure!(persisted.0[..] == key[..], "Keychain database key verification failed");
                Ok((persisted, true))
            }
            Err(error) => Err(error).context(
                "database Keychain key missing, locked, or inaccessible; no replacement or plaintext fallback",
            ),
        }
    }

    #[cfg(test)]
    #[test]
    fn isolated_keychain_roundtrip_and_loss_fail_closed() {
        use security_framework::os::macos::keychain::CreateOptions;
        let directory = tempfile::tempdir().unwrap();
        let keychain = CreateOptions::new()
            .password("fixture-only-password")
            .create(directory.path().join("fixture.keychain"))
            .unwrap();
        let (created, fresh) = load(&keychain, "fixture", true).unwrap();
        assert!(fresh);
        let (loaded, fresh) = load(&keychain, "fixture", false).unwrap();
        assert!(!fresh);
        assert_eq!(created.0[..], loaded.0[..]);
        let (_, item) =
            find_generic_password(Some(&[keychain.clone()]), SERVICE, "fixture").unwrap();
        item.delete();
        assert!(load(&keychain, "fixture", false).is_err());
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use rand::{rngs::OsRng, RngCore};
    use std::fs::{self, OpenOptions};
    use std::io::{Read, Write};
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    use zeroize::Zeroize;

    const FORMAT: &[u8] = b"FocusBridge-DPAPI-key-v1\0";
    const PAYLOAD: &[u8] = b"FocusBridge-SQLCipher-256-v1\0";
    const MAX_ENVELOPE: u64 = 16 * 1024;

    pub(super) fn load_or_create(path: &Path, allow_create: bool) -> Result<DatabaseKey> {
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                ensure!(
                    metadata.file_type().is_file(),
                    "database key must be a regular file"
                );
                let mut envelope = Vec::new();
                fs::File::open(path)?
                    .take(MAX_ENVELOPE + 1)
                    .read_to_end(&mut envelope)?;
                ensure!(
                    envelope.len() as u64 <= MAX_ENVELOPE,
                    "invalid database key envelope size"
                );
                ensure!(
                    envelope.starts_with(FORMAT),
                    "unsupported or corrupt database key envelope; key was not reset"
                );
                let plaintext = protect(&envelope[FORMAT.len()..], false)
                    .context("cannot unwrap database key for this Windows user; preserve database and key for recovery")?;
                ensure!(
                    plaintext.len() == PAYLOAD.len() + 32 && plaintext.starts_with(PAYLOAD),
                    "invalid unwrapped database key; key was not reset"
                );
                let mut key = Zeroizing::new([0u8; 32]);
                key.copy_from_slice(&plaintext[PAYLOAD.len()..]);
                Ok(DatabaseKey(key))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && allow_create => {
                let mut key = Zeroizing::new([0u8; 32]);
                OsRng
                    .try_fill_bytes(&mut *key)
                    .context("generate database key from OS entropy")?;
                let mut plaintext = Zeroizing::new(Vec::with_capacity(PAYLOAD.len() + 32));
                plaintext.extend_from_slice(PAYLOAD);
                plaintext.extend_from_slice(&*key);
                let wrapped = protect(&plaintext, true)
                    .context("wrap database key with Windows user DPAPI")?;
                // CREATE_NEW never replaces an existing envelope. A partial write is
                // deliberately a recovery error on the next launch, not a key reset.
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .context("create protected database key without overwriting an existing key")?;
                file.write_all(FORMAT)?;
                file.write_all(&wrapped)?;
                file.sync_all()
                    .context("flush protected database key before creating encrypted data")?;
                drop(file);
                let persisted = load_or_create(path, false)
                    .context("verify persisted database key before encrypting any data")?;
                ensure!(
                    persisted.0[..] == key[..],
                    "persisted database key does not match; no data was encrypted"
                );
                Ok(persisted)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                bail!("database key is missing; encrypted data or migration files exist, so no replacement key will be generated")
            }
            Err(error) => Err(error).context("read protected database key"),
        }
    }

    fn protect(input: &[u8], encrypt: bool) -> Result<Zeroizing<Vec<u8>>> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: input.len().try_into().context("DPAPI input size")?,
            pbData: input.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: null_mut(),
        };
        // UI_FORBIDDEN permits background sync. In particular, never set
        // CRYPTPROTECT_LOCAL_MACHINE: other Windows users must not unwrap keys.
        let success = unsafe {
            if encrypt {
                CryptProtectData(
                    &input,
                    null(),
                    null(),
                    null(),
                    null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output,
                )
            } else {
                CryptUnprotectData(
                    &input,
                    null_mut(),
                    null(),
                    null(),
                    null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output,
                )
            }
        };
        if success == 0 {
            return Err(std::io::Error::last_os_error()).context("Windows DPAPI operation failed");
        }
        ensure!(!output.pbData.is_null(), "Windows DPAPI returned no output");
        // DPAPI owns this allocation. Wipe both decrypted output and the Rust
        // copy, including on validation errors, before releasing their memory.
        let result = unsafe {
            let bytes = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
            let result = Zeroizing::new(bytes.to_vec());
            bytes.zeroize();
            LocalFree(output.pbData.cast());
            result
        };
        Ok(result)
    }

    #[cfg(test)]
    #[test]
    fn dpapi_round_trip_never_persists_the_explicit_test_key() {
        let key = [0x63u8; 32];
        let wrapped = protect(&key, true).unwrap();
        assert!(!wrapped.windows(key.len()).any(|bytes| bytes == key));
        let unwrapped = protect(&wrapped, false).unwrap();
        assert_eq!(unwrapped.as_slice(), key);
    }
}
