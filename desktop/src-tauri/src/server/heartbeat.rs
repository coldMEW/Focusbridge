use tokio::time::{Duration, Instant};

pub const PROBE_INTERVAL: Duration = Duration::from_secs(3);
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(6);

/// Transport probes are independent of Android's slower application heartbeat.
pub struct PeerHeartbeat {
    next_probe: Instant,
    pending: Option<(Vec<u8>, Instant)>,
    sequence: u64,
}

impl PeerHeartbeat {
    pub fn new(now: Instant) -> Self {
        Self {
            next_probe: now,
            pending: None,
            sequence: 0,
        }
    }

    pub fn deadline(&self) -> Instant {
        self.pending
            .as_ref()
            .map(|(_, deadline)| *deadline)
            .unwrap_or(self.next_probe + PROBE_TIMEOUT)
    }

    pub fn probe(&mut self, now: Instant) -> Option<Vec<u8>> {
        if self.pending.is_some() || now < self.next_probe || now >= self.deadline() {
            return None;
        }
        self.sequence = self.sequence.wrapping_add(1);
        let token = self.sequence.to_be_bytes().to_vec();
        self.pending = Some((token.clone(), now + PROBE_TIMEOUT));
        Some(token)
    }

    pub fn wake_at(&self) -> Instant {
        if self.pending.is_some() {
            self.deadline()
        } else {
            self.next_probe
        }
    }

    pub fn acknowledge(&mut self, payload: &[u8], now: Instant) -> bool {
        let valid = self
            .pending
            .as_ref()
            .map(|(token, deadline)| token == payload && now < *deadline)
            .unwrap_or(false);
        if valid {
            self.pending = None;
            self.next_probe = now + PROBE_INTERVAL;
        }
        valid
    }
}
