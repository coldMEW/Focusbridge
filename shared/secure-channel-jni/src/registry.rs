use crate::{Error, Result};
use focusbridge_secure_channel::Session;
use std::collections::BTreeMap;

pub(crate) const MAX_SESSIONS: usize = 32;

pub(crate) struct Registry {
    next: i64,
    sessions: BTreeMap<i64, Session>,
}

impl Registry {
    pub(crate) fn new() -> Self {
        Self {
            next: 1,
            sessions: BTreeMap::new(),
        }
    }

    pub(crate) fn insert_with(&mut self, create: impl FnOnce() -> Result<Session>) -> Result<i64> {
        if self.sessions.len() >= MAX_SESSIONS || self.next <= 0 {
            return Err(Error::Capacity);
        }
        let session = std::panic::catch_unwind(std::panic::AssertUnwindSafe(create))
            .map_err(|_| Error::Internal)??;
        let handle = self.next;
        self.next = self.next.checked_add(1).unwrap_or(0);
        self.sessions.insert(handle, session);
        Ok(handle)
    }

    pub(crate) fn with_session<T>(
        &mut self,
        handle: i64,
        operation: impl FnOnce(&mut Session) -> Result<T>,
    ) -> Result<T> {
        let session = self.sessions.get_mut(&handle).ok_or(Error::InvalidHandle)?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(session)))
            .unwrap_or(Err(Error::Internal));
        if result.is_err() {
            self.sessions.remove(&handle);
        }
        result
    }

    pub(crate) fn close(&mut self, handle: i64) {
        self.sessions.remove(&handle);
    }

    pub(crate) fn close_all(&mut self) {
        self.sessions.clear();
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
