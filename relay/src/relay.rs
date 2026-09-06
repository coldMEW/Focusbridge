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
    // Relay heartbeat scheduling is handled by the WebSocket task.
    pub ping_interval: Duration,
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
            while let Some(front) = sess.pending.front() {
                if let Some(desktop) = &sess.desktop_tx {
                    if desktop.send(front.body.clone()).is_err() {
                        sess.desktop_tx = None;
                        break;
                    }
                    sess.pending.pop_front();
                } else {
                    break;
                }
            }
        }
        true
    }

    pub async fn detach(&self, pair_id: &str, role: Role, sender: &Tx) {
        if let Some(entry) = self
            .sessions
            .get(pair_id)
            .map(|entry| Arc::clone(entry.value()))
        {
            let mut sess = entry.lock().await;
            if !Self::owns_session(&sess, role, sender) {
                return;
            }
            match role {
                Role::Android => sess.android_tx = None,
                Role::Desktop => sess.desktop_tx = None,
            }
            sess.touch();
        }
    }

    fn owns_session(sess: &Session, role: Role, sender: &Tx) -> bool {
        let current = match role {
            Role::Android => &sess.android_tx,
            Role::Desktop => &sess.desktop_tx,
        };
        current
            .as_ref()
            .is_some_and(|active| active.same_channel(sender))
    }

    pub async fn is_current(&self, pair_id: &str, role: Role, sender: &Tx) -> bool {
        let Some(entry) = self
            .sessions
            .get(pair_id)
            .map(|entry| Arc::clone(entry.value()))
        else {
            return false;
        };
        let sess = entry.lock().await;
        Self::owns_session(&sess, role, sender)
    }

    pub async fn route_from(
        &self,
        pair_id: &str,
        role: Role,
        sender: &Tx,
        body: String,
    ) -> RouteResult {
        let Some(entry) = self
            .sessions
            .get(pair_id)
            .map(|entry| Arc::clone(entry.value()))
        else {
            return RouteResult::NoSession;
        };
        let mut sess = entry.lock().await;
        if !Self::owns_session(&sess, role, sender) {
            return RouteResult::StaleConnection;
        }
        if !sess.allow_message_at(role, self.rate_limit_per_min, std::time::Instant::now()) {
            return RouteResult::RateLimited;
        }
        sess.touch();
        match role {
            Role::Android => self.send_to_desktop(&mut sess, body),
            Role::Desktop => self.send_to_android(&mut sess, body),
        }
    }

    /// Android → Desktop. Returns true if delivered live, false if queued.
    fn send_to_desktop(&self, sess: &mut Session, body: String) -> RouteResult {
        if body.len() > self.max_message_bytes {
            return RouteResult::TooLarge;
        }
        let body = if let Some(desktop) = &sess.desktop_tx {
            match desktop.send(body) {
                Ok(()) => return RouteResult::Delivered,
                Err(error) => error.0,
            }
        } else {
            body
        };
        sess.desktop_tx = None;
        sess.expire_pending(self.pending_ttl);
        sess.enqueue_pending(body, self.max_pending);
        RouteResult::Queued
    }

    fn send_to_android(&self, sess: &mut Session, body: String) -> RouteResult {
        if body.len() > self.max_message_bytes {
            return RouteResult::TooLarge;
        }
        if let Some(android) = &sess.android_tx {
            if android.send(body).is_ok() {
                return RouteResult::Delivered;
            }
        }
        sess.android_tx = None;
        RouteResult::Dropped
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RouteResult {
    Delivered,
    Queued,
    Dropped,
    NoSession,
    TooLarge,
    StaleConnection,
    RateLimited,
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
    async fn failed_desktop_send_keeps_message_for_reconnect() {
        let state = new_state();
        let (phone, _phone_rx) = unbounded_channel();
        state
            .attach("p", "k".into(), Role::Android, phone.clone())
            .await;
        let (tx, rx) = unbounded_channel();
        state.attach("p", "k".into(), Role::Desktop, tx).await;
        drop(rx);
        assert_eq!(
            state
                .route_from("p", Role::Android, &phone, "keep".into())
                .await,
            RouteResult::Queued
        );
        let (tx, mut rx) = unbounded_channel();
        state.attach("p", "k".into(), Role::Desktop, tx).await;
        assert_eq!(rx.try_recv().unwrap(), "keep");
    }

    #[tokio::test]
    async fn failed_pending_flush_preserves_queue() {
        let state = new_state();
        let (tx, _rx) = unbounded_channel();
        state
            .attach("p", "k".into(), Role::Android, tx.clone())
            .await;
        state
            .route_from("p", Role::Android, &tx, "keep".into())
            .await;
        let (tx, rx) = unbounded_channel();
        drop(rx);
        state.attach("p", "k".into(), Role::Desktop, tx).await;
        let (tx, mut rx) = unbounded_channel();
        state.attach("p", "k".into(), Role::Desktop, tx).await;
        assert_eq!(rx.try_recv().unwrap(), "keep");
    }

    #[tokio::test]
    async fn closed_phone_channel_is_not_reported_as_delivered() {
        let state = new_state();
        let (desktop, _desktop_rx) = unbounded_channel();
        state
            .attach("p", "k".into(), Role::Desktop, desktop.clone())
            .await;
        let (tx, rx) = unbounded_channel();
        state.attach("p", "k".into(), Role::Android, tx).await;
        drop(rx);
        assert_eq!(
            state
                .route_from("p", Role::Desktop, &desktop, "command".into())
                .await,
            RouteResult::Dropped
        );
    }

    #[tokio::test]
    async fn stale_detach_does_not_remove_replacement() {
        let state = new_state();
        let (phone, _phone_rx) = unbounded_channel();
        state
            .attach("p", "k".into(), Role::Android, phone.clone())
            .await;
        let (old, _old_rx) = unbounded_channel();
        let (new, mut new_rx) = unbounded_channel();
        state
            .attach("p", "k".into(), Role::Desktop, old.clone())
            .await;
        state.attach("p", "k".into(), Role::Desktop, new).await;
        state.detach("p", Role::Desktop, &old).await;
        assert_eq!(
            state
                .route_from("p", Role::Android, &phone, "new".into())
                .await,
            RouteResult::Delivered
        );
        assert_eq!(new_rx.try_recv().unwrap(), "new");
    }

    #[tokio::test]
    async fn replaced_senders_cannot_route_in_either_direction() {
        for role in [Role::Android, Role::Desktop] {
            let state = new_state();
            let (old, _old_rx) = unbounded_channel();
            let (new, _new_rx) = unbounded_channel();
            state.attach("p", "k".into(), role, old.clone()).await;
            state.attach("p", "k".into(), role, new.clone()).await;
            assert!(!state.is_current("p", role, &old).await);
            assert!(state.is_current("p", role, &new).await);
            assert_eq!(
                state.route_from("p", role, &old, "stale".into()).await,
                RouteResult::StaleConnection
            );
            state.detach("p", role, &old).await;
            assert!(state.is_current("p", role, &new).await);
            state.detach("p", role, &new).await;
            assert!(!state.is_current("p", role, &new).await);
        }
    }

    #[tokio::test]
    async fn reconnect_does_not_reset_message_budget() {
        let state = RelayState::new(Duration::from_secs(300), 65536, Duration::from_secs(30), 1);
        let (old, _rx) = unbounded_channel();
        state
            .attach("p", "k".into(), Role::Android, old.clone())
            .await;
        assert_eq!(
            state
                .route_from("p", Role::Android, &old, "one".into())
                .await,
            RouteResult::Queued
        );
        let (new, _rx) = unbounded_channel();
        state
            .attach("p", "k".into(), Role::Android, new.clone())
            .await;
        assert_eq!(
            state
                .route_from("p", Role::Android, &new, "two".into())
                .await,
            RouteResult::RateLimited
        );
    }

    #[tokio::test]
    async fn android_to_desktop_delivers_when_both_attached() {
        let state = new_state();
        let (atx, _arx) = unbounded_channel::<String>();
        let (dtx, mut drx) = unbounded_channel::<String>();
        assert!(
            state
                .attach("p1", "k".into(), Role::Android, atx.clone())
                .await
        );
        assert!(state.attach("p1", "k".into(), Role::Desktop, dtx).await);
        assert_eq!(
            state
                .route_from("p1", Role::Android, &atx, "hello".into())
                .await,
            RouteResult::Delivered
        );
        assert_eq!(drx.recv().await.unwrap(), "hello");
    }

    #[tokio::test]
    async fn android_queues_when_desktop_absent_then_flushes_on_attach() {
        let state = new_state();
        let (atx, _arx) = unbounded_channel::<String>();
        assert!(
            state
                .attach("p2", "k".into(), Role::Android, atx.clone())
                .await
        );
        assert_eq!(
            state
                .route_from("p2", Role::Android, &atx, "q1".into())
                .await,
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
        state
            .attach("p4", "k".into(), Role::Android, atx.clone())
            .await;
        state.attach("p4", "k".into(), Role::Desktop, dtx).await;
        assert_eq!(
            state
                .route_from("p4", Role::Android, &atx, "x".repeat(64))
                .await,
            RouteResult::TooLarge
        );
    }
}
