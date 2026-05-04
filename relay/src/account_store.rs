use crate::auth::{
    create_password_hash, normalize_email, verify_password, AuthError, PasswordHash,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
        data.email_index.insert(email, user.id.clone());
        data.users.insert(user.id.clone(), user.clone());
        self.save_locked(&data).map_err(RegisterError::Store)?;
        Ok(user)
    }

    pub fn login_password(&self, email: &str, password: &str) -> Result<UserAccount, LoginError> {
        let email = normalize_email(email).map_err(|_| LoginError::InvalidCredentials)?;
        let data = self.data.lock().expect("account store mutex poisoned");
        let user_id = data
            .email_index
            .get(&email)
            .ok_or(LoginError::InvalidCredentials)?;
        let user = data
            .users
            .get(user_id)
            .ok_or(LoginError::InvalidCredentials)?;
        let Some(hash) = &user.password else {
            return Err(LoginError::InvalidCredentials);
        };
        if verify_password(password, hash).map_err(|_| LoginError::InvalidCredentials)? {
            Ok(user.clone())
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
        data.email_index.insert(email, user.id.clone());
        data.users.insert(user.id.clone(), user.clone());
        self.save_locked(&data).map_err(RegisterError::Store)?;
        Ok(user)
    }

    fn save_locked(&self, data: &StoreData) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create account store directory {}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(data).context("serialize account store")?;
        std::fs::write(&self.path, text)
            .with_context(|| format!("write account store {}", self.path.display()))
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
