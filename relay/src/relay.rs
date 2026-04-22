use crate::auth::Role;
use crate::session::{Session, Tx};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub type SessionMap = DashMap<String, Arc<Mutex<Session>>>;

pub struct RelayState {
    pub sessions: SessionMap,
    pub max_pending: usize,
    pub pending_ttl: Duration,
    pub max_message_bytes: usize,
    // Reserved for ping scheduler + rate limiter (future wiring).
    #[allow(dead_code)]
    pub ping_interval: Duration,
    #[allow(dead_code)]
    pub rate_limit_per_min: u32,
}

impl RelayState {
    pub fn new(
        pending_ttl: Duration,
        max_message_bytes: usize,
        ping_interval: Duration,
        rate_limit_per_min: u32,
    ) -> Self {
        Self {
            sessions: DashMap::new(),
            max_pending: 100,
            pending_ttl,
            max_message_bytes,
            ping_interval,
            rate_limit_per_min,
        }
    }

    pub async fn attach(&self, pair_id: &str, pairing_key: String, role: Role, tx: Tx) -> bool {
        let entry = self
            .sessions
            .entry(pair_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(Session::new(pairing_key.clone()))))
            .clone();
        let mut sess = entry.lock().await;

        if sess.pairing_key != pairing_key {
            return false;
        }
        sess.touch();
        match role {
            Role::Android => sess.android_tx = Some(tx),
            Role::Desktop => sess.desktop_tx = Some(tx),
        }

        // Flush pending on desktop attach.
        if matches!(role, Role::Desktop) {
            sess.expire_pending(self.pending_ttl);
            while let Some(front) = sess.pending.pop_front() {
                if let Some(desktop) = &sess.desktop_tx {
                    let _ = desktop.send(front.body);
                }
            }
        }
        true
    }

    pub async fn detach(&self, pair_id: &str, role: Role) {
        if let Some(entry) = self.sessions.get(pair_id) {
            let entry = entry.clone();
            let mut sess = entry.lock().await;
            match role {
                Role::Android => sess.android_tx = None,
                Role::Desktop => sess.desktop_tx = None,
            }
            sess.touch();
        }
    }

    /// Android → Desktop. Returns true if delivered live, false if queued.
    pub async fn route_android_to_desktop(&self, pair_id: &str, body: String) -> RouteResult {
        let Some(entry) = self.sessions.get(pair_id) else {
            return RouteResult::NoSession;
        };
        let entry = entry.clone();
        let mut sess = entry.lock().await;
        sess.touch();
        if body.len() > self.max_message_bytes {
            return RouteResult::TooLarge;
        }
        if let Some(desktop) = &sess.desktop_tx {
            let _ = desktop.send(body);
            RouteResult::Delivered
        } else {
            sess.expire_pending(self.pending_ttl);
            sess.enqueue_pending(body, self.max_pending);
            RouteResult::Queued
        }
    }

    pub async fn route_desktop_to_android(&self, pair_id: &str, body: String) -> RouteResult {
        let Some(entry) = self.sessions.get(pair_id) else {
            return RouteResult::NoSession;
        };
        let entry = entry.clone();
        let mut sess = entry.lock().await;
        sess.touch();
        if body.len() > self.max_message_bytes {
            return RouteResult::TooLarge;
        }
        if let Some(android) = &sess.android_tx {
            let _ = android.send(body);
            RouteResult::Delivered
        } else {
            RouteResult::Dropped
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RouteResult {
    Delivered,
    Queued,
    Dropped,
    NoSession,
    TooLarge,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::unbounded_channel;

    fn new_state() -> RelayState {
        RelayState::new(
            Duration::from_secs(300),
            65536,
            Duration::from_secs(30),
            120,
        )
    }

    #[tokio::test]
    async fn android_to_desktop_delivers_when_both_attached() {
        let state = new_state();
        let (atx, _arx) = unbounded_channel::<String>();
        let (dtx, mut drx) = unbounded_channel::<String>();
        assert!(state.attach("p1", "k".into(), Role::Android, atx).await);
        assert!(state.attach("p1", "k".into(), Role::Desktop, dtx).await);
        assert_eq!(
            state.route_android_to_desktop("p1", "hello".into()).await,
            RouteResult::Delivered
        );
        assert_eq!(drx.recv().await.unwrap(), "hello");
    }

    #[tokio::test]
    async fn android_queues_when_desktop_absent_then_flushes_on_attach() {
        let state = new_state();
        let (atx, _arx) = unbounded_channel::<String>();
        assert!(state.attach("p2", "k".into(), Role::Android, atx).await);
        assert_eq!(
            state.route_android_to_desktop("p2", "q1".into()).await,
            RouteResult::Queued
        );
        let (dtx, mut drx) = unbounded_channel::<String>();
        assert!(state.attach("p2", "k".into(), Role::Desktop, dtx).await);
        assert_eq!(drx.recv().await.unwrap(), "q1");
    }

    #[tokio::test]
    async fn wrong_pairing_key_rejects() {
        let state = new_state();
        let (tx, _rx) = unbounded_channel::<String>();
        assert!(state.attach("p3", "k1".into(), Role::Android, tx).await);
        let (tx2, _rx2) = unbounded_channel::<String>();
        assert!(!state.attach("p3", "k2".into(), Role::Desktop, tx2).await);
    }

    #[tokio::test]
    async fn oversize_message_rejected() {
        let state = RelayState::new(Duration::from_secs(300), 16, Duration::from_secs(30), 120);
        let (atx, _arx) = unbounded_channel::<String>();
        let (dtx, _drx) = unbounded_channel::<String>();
        state.attach("p4", "k".into(), Role::Android, atx).await;
        state.attach("p4", "k".into(), Role::Desktop, dtx).await;
        assert_eq!(
            state.route_android_to_desktop("p4", "x".repeat(64)).await,
            RouteResult::TooLarge
        );
    }
}
