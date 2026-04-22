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
        if self.pending.len() >= max_len {
            self.pending.pop_front();
        }
        self.pending.push_back(PendingMessage {
            body,
            enqueued_at: Instant::now(),
        });
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
}
