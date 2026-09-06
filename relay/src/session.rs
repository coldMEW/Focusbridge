use crate::auth::Role;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub type Tx = mpsc::UnboundedSender<String>;

pub struct Session {
    pub pairing_key: String,
    pub android_tx: Option<Tx>,
    pub desktop_tx: Option<Tx>,
    pub pending: VecDeque<PendingMessage>,
    pub last_active: Instant,
    message_windows: [(Instant, u32); 2],
}

pub struct PendingMessage {
    pub body: String,
    pub enqueued_at: Instant,
}

impl Session {
    pub fn new(pairing_key: String) -> Self {
        Self {
            pairing_key,
            android_tx: None,
            desktop_tx: None,
            pending: VecDeque::new(),
            last_active: Instant::now(),
            message_windows: [(Instant::now(), 0); 2],
        }
    }

    pub fn touch(&mut self) {
        self.last_active = Instant::now();
    }

    /// Drop expired items. Max size cap enforced separately on enqueue.
    pub fn expire_pending(&mut self, ttl: Duration) {
        let now = Instant::now();
        while let Some(front) = self.pending.front() {
            if now.duration_since(front.enqueued_at) > ttl {
                self.pending.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn enqueue_pending(&mut self, body: String, max_len: usize) {
        if max_len == 0 {
            return;
        }
        if self.pending.len() >= max_len {
            self.pending.pop_front();
        }
        self.pending.push_back(PendingMessage {
            body,
            enqueued_at: Instant::now(),
        });
    }

    pub fn allow_message_at(&mut self, role: Role, limit: u32, now: Instant) -> bool {
        let index = match role {
            Role::Android => 0,
            Role::Desktop => 1,
        };
        let (started, count) = &mut self.message_windows[index];
        if now.duration_since(*started) >= Duration::from_secs(60) {
            *started = now;
            *count = 0;
        }
        if *count >= limit {
            return false;
        }
        *count += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_respects_cap() {
        let mut s = Session::new("k".into());
        for i in 0..5 {
            s.enqueue_pending(format!("m{i}"), 3);
        }
        assert_eq!(s.pending.len(), 3);
        assert_eq!(s.pending.front().unwrap().body, "m2");
    }

    #[test]
    fn zero_capacity_retains_nothing() {
        let mut session = Session::new("key".into());
        session.enqueue_pending("message".into(), 0);
        assert!(session.pending.is_empty());
    }

    #[test]
    fn message_budget_is_per_role_and_resets_after_one_minute() {
        let mut session = Session::new("key".into());
        let now = Instant::now();
        assert!(session.allow_message_at(crate::auth::Role::Android, 1, now));
        assert!(!session.allow_message_at(crate::auth::Role::Android, 1, now));
        assert!(session.allow_message_at(crate::auth::Role::Desktop, 1, now));
        assert!(session.allow_message_at(
            crate::auth::Role::Android,
            1,
            now + Duration::from_secs(60)
        ));
    }
}
