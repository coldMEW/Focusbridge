use crate::auth::{
    create_password_hash, normalize_email, verify_password, AuthError, PasswordHash,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAccount {
    pub id: String,
    pub email: String,
    pub password: Option<PasswordHash>,
    pub provider: AuthProvider,
    pub created_at_epoch_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthProvider {
    Password,
    Google,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct StoreData {
    users: BTreeMap<String, UserAccount>,
    email_index: BTreeMap<String, String>,
}

pub struct AccountStore {
    path: PathBuf,
    data: Mutex<StoreData>,
}

impl AccountStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let data = if path.exists() {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("read account store {}", path.display()))?;
            serde_json::from_str(&text)
                .with_context(|| format!("parse account store {}", path.display()))?
        } else {
            StoreData::default()
        };
        Ok(Self {
            path,
            data: Mutex::new(data),
        })
    }

    pub fn register_password(
        &self,
        email: &str,
        password: &str,
        now_epoch_secs: u64,
    ) -> Result<UserAccount, RegisterError> {
        let email = normalize_email(email).map_err(RegisterError::Auth)?;
        let password = create_password_hash(password).map_err(RegisterError::Auth)?;
        let mut data = self.data.lock().expect("account store mutex poisoned");
        if data.email_index.contains_key(&email) {
            return Err(RegisterError::AlreadyExists);
        }
        let user = UserAccount {
            id: format!("user_{}", uuid::Uuid::new_v4().simple()),
            email: email.clone(),
            password: Some(password),
            provider: AuthProvider::Password,
            created_at_epoch_secs: now_epoch_secs,
        };
        let mut next = data.clone();
        next.email_index.insert(email, user.id.clone());
        next.users.insert(user.id.clone(), user.clone());
        self.save_locked(&next).map_err(RegisterError::Store)?;
        *data = next;
        Ok(user)
    }

    pub fn login_password(&self, email: &str, password: &str) -> Result<UserAccount, LoginError> {
        let email = normalize_email(email).map_err(|_| LoginError::InvalidCredentials)?;
        let user = {
            let data = self.data.lock().expect("account store mutex poisoned");
            let user_id = data
                .email_index
                .get(&email)
                .ok_or(LoginError::InvalidCredentials)?;
            data.users
                .get(user_id)
                .cloned()
                .ok_or(LoginError::InvalidCredentials)?
        };
        // Accounts are not updated or removed, so this snapshot remains valid.
        let Some(hash) = &user.password else {
            return Err(LoginError::InvalidCredentials);
        };
        if verify_password(password, hash).map_err(|_| LoginError::InvalidCredentials)? {
            Ok(user)
        } else {
            Err(LoginError::InvalidCredentials)
        }
    }

    pub fn upsert_google_user(
        &self,
        email: &str,
        now_epoch_secs: u64,
    ) -> Result<UserAccount, RegisterError> {
        let email = normalize_email(email).map_err(RegisterError::Auth)?;
        let mut data = self.data.lock().expect("account store mutex poisoned");
        if let Some(user_id) = data.email_index.get(&email) {
            return data
                .users
                .get(user_id)
                .cloned()
                .ok_or_else(|| RegisterError::Store(anyhow::anyhow!("email index is stale")));
        }
        let user = UserAccount {
            id: format!("user_{}", uuid::Uuid::new_v4().simple()),
            email: email.clone(),
            password: None,
            provider: AuthProvider::Google,
            created_at_epoch_secs: now_epoch_secs,
        };
        let mut next = data.clone();
        next.email_index.insert(email, user.id.clone());
        next.users.insert(user.id.clone(), user.clone());
        self.save_locked(&next).map_err(RegisterError::Store)?;
        *data = next;
        Ok(user)
    }

    fn save_locked(&self, data: &StoreData) -> Result<()> {
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create account store directory {}", parent.display()))?;
        let text = serde_json::to_string_pretty(data).context("serialize account store")?;
        let temporary = parent.join(format!(
            ".account-store-{}.tmp",
            uuid::Uuid::new_v4().simple()
        ));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .context("create temporary account store")?;
        let result = (|| -> Result<()> {
            file.write_all(text.as_bytes())
                .context("write temporary account store")?;
            file.sync_all().context("sync temporary account store")?;
            drop(file);
            std::fs::rename(&temporary, &self.path)
                .with_context(|| format!("replace account store {}", self.path.display()))?;
            Ok(())
        })();
        if result.is_err() {
            if let Err(error) = std::fs::remove_file(&temporary) {
                tracing::warn!(%error, "could not clean up temporary account store");
            }
        }
        result?;
        // Rename commits the transaction. A later directory-sync failure must not
        // leave memory behind the file that has already been published.
        #[cfg(unix)]
        if let Err(error) = std::fs::File::open(parent).and_then(|dir| dir.sync_all()) {
            tracing::warn!(%error, "could not sync account store directory after replacement");
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RegisterError {
    #[error("account already exists")]
    AlreadyExists,
    #[error("{0}")]
    Auth(AuthError),
    #[error(transparent)]
    Store(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("invalid email or password")]
    InvalidCredentials,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "focusbridge-account-store-{}",
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn failed_registration_does_not_publish(password: bool) {
        let directory = TestDirectory::new();
        let path = directory.0.join("accounts.json");
        let store = AccountStore::load(&path).unwrap();
        let existing = store.upsert_google_user("existing@example.com", 1).unwrap();
        let before = std::fs::read(&path).unwrap();
        let backup = directory.0.join("backup.json");
        std::fs::rename(&path, &backup).unwrap();
        std::fs::create_dir(&path).unwrap();

        let result = if password {
            store.register_password("new@example.com", "correct horse battery staple", 2)
        } else {
            store.upsert_google_user("new@example.com", 2)
        };
        assert!(matches!(result, Err(RegisterError::Store(_))));
        let data = store.data.lock().unwrap();
        assert_eq!(data.users.len(), 1, "failed save published a user");
        assert_eq!(data.email_index.len(), 1, "failed save published an email");
        assert!(data.users.contains_key(&existing.id));
        drop(data);
        assert_eq!(std::fs::read(&backup).unwrap(), before);
        assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 2);

        std::fs::remove_dir(&path).unwrap();
        std::fs::rename(&backup, &path).unwrap();
        let user = if password {
            assert!(store
                .login_password("new@example.com", "correct horse battery staple")
                .is_err());
            store.register_password("new@example.com", "correct horse battery staple", 2)
        } else {
            store.upsert_google_user("new@example.com", 2)
        }
        .unwrap();
        let reloaded = AccountStore::load(&path).unwrap();
        assert!(reloaded.data.lock().unwrap().users.contains_key(&user.id));
    }

    #[test]
    fn failed_password_registration_does_not_publish() {
        failed_registration_does_not_publish(true);
    }

    #[test]
    fn failed_google_registration_does_not_publish() {
        failed_registration_does_not_publish(false);
    }

    #[test]
    fn save_replaces_file_without_overwriting_previous_snapshot() {
        let directory = TestDirectory::new();
        let path = directory.0.join("accounts.json");
        let store = AccountStore::load(&path).unwrap();
        let first = store.upsert_google_user("first@example.com", 1).unwrap();
        let before = std::fs::read(&path).unwrap();
        let snapshot = directory.0.join("snapshot.json");
        std::fs::hard_link(&path, &snapshot).unwrap();
        let second = store.upsert_google_user("second@example.com", 2).unwrap();

        assert_eq!(
            std::fs::read(&snapshot).unwrap(),
            before,
            "save overwrote the previous file"
        );
        let reloaded = AccountStore::load(&path).unwrap();
        let data = reloaded.data.lock().unwrap();
        assert_eq!(data.users.len(), 2);
        assert!(data.users.contains_key(&first.id));
        assert!(data.users.contains_key(&second.id));
        assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 2);
    }

    #[test]
    fn password_registration_login_and_persistence_work() {
        let path = std::env::temp_dir().join(format!(
            "focusbridge-auth-test-{}.json",
            uuid::Uuid::new_v4().simple()
        ));
        let store = AccountStore::load(&path).unwrap();
        let user = store
            .register_password("User@Example.com", "correct horse battery staple", 1_000)
            .unwrap();

        assert_eq!(user.email, "user@example.com");
        assert_eq!(
            store
                .login_password("user@example.com", "correct horse battery staple")
                .unwrap()
                .id,
            user.id
        );
        assert!(store
            .login_password("user@example.com", "wrong password")
            .is_err());

        let reloaded = AccountStore::load(&path).unwrap();
        assert_eq!(
            reloaded
                .login_password("user@example.com", "correct horse battery staple")
                .unwrap()
                .id,
            user.id
        );
        let _ = std::fs::remove_file(path);
    }
}
