use tokio::time::{Duration, Instant};

/// How often the transport is probed while the session is otherwise idle.
pub const PROBE_INTERVAL: Duration = Duration::from_secs(15);
/// How long one probe waits for its pong before it is simply sent again.
///
/// A late probe is not a dead phone. A handset in Doze, on power-saving Wi-Fi,
/// or reached through the relay routinely takes seconds to turn its radio
/// around.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(20);
/// Total silence -- no pong, no message, nothing at all -- before the phone is
/// declared gone.
///
/// Six missed probes, and inside the 180s this desktop advertises to the phone
/// in `AUTH_OK`, so the two ends agree about when a session has ended rather
/// than the desktop ending it thirty times sooner than it said it would.
pub const SESSION_TIMEOUT: Duration = Duration::from_secs(90);

/// Transport probes are independent of Android's slower application heartbeat.
///
/// The session used to end on the first probe left unanswered for six seconds,
/// while the phone had been told it had a hundred and eighty. So an ordinary
/// radio hiccup dropped a healthy connection, and a session lasted anywhere
/// from seconds to hours with both devices plainly online. Liveness is the
/// peer's *silence* now, not one late reply, and an application frame from the
/// phone proves it is there just as a matching pong does.
pub struct PeerHeartbeat {
    next_probe: Instant,
    pending: Option<(Vec<u8>, Instant)>,
    last_seen: Instant,
    sequence: u64,
}

impl PeerHeartbeat {
    pub fn new(now: Instant) -> Self {
        Self {
            next_probe: now,
            pending: None,
            last_seen: now,
            sequence: 0,
        }
    }

    /// The instant this session is considered dead. Only silence moves it.
    pub fn deadline(&self) -> Instant {
        self.last_seen + SESSION_TIMEOUT
    }

    /// Any frame from the phone proves the transport is alive.
    pub fn mark_alive(&mut self, now: Instant) {
        if now > self.last_seen {
            self.last_seen = now;
        }
    }

    pub fn probe(&mut self, now: Instant) -> Option<Vec<u8>> {
        if let Some(expires) = self.pending.as_ref().map(|(_, expires)| *expires) {
            if now < expires {
                return None;
            }
            // This probe went unanswered. Send another one; whether the phone is
            // gone is decided by `deadline`, not here.
            self.pending = None;
            self.next_probe = now;
        }
        if now < self.next_probe {
            return None;
        }
        self.sequence = self.sequence.wrapping_add(1);
        let token = self.sequence.to_be_bytes().to_vec();
        self.pending = Some((token.clone(), now + PROBE_TIMEOUT));
        self.next_probe = now + PROBE_INTERVAL;
        Some(token)
    }

    pub fn wake_at(&self) -> Instant {
        match self.pending.as_ref() {
            // While a probe is outstanding there is nothing to send until it
            // lapses; waking before then would spin the caller's loop.
            Some((_, expires)) => *expires,
            None => self.next_probe,
        }
    }

    /// Closes the outstanding probe, if this is the pong it was waiting for.
    ///
    /// Only a pong matching the probe still in flight counts. A challenge is
    /// worth nothing if any stray or long-late pong answers it, so an unmatched
    /// one is not treated as proof of life -- unlike an application frame,
    /// which the phone can only have sent itself.
    pub fn acknowledge(&mut self, payload: &[u8], now: Instant) -> bool {
        let answered = self
            .pending
            .as_ref()
            .map(|(token, expires)| token.as_slice() == payload && now < *expires)
            .unwrap_or(false);
        if answered {
            self.pending = None;
            self.next_probe = now + PROBE_INTERVAL;
            self.mark_alive(now);
        }
        answered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_late_pong_does_not_end_the_session() {
        // The bug this exists for: one probe unanswered for six seconds closed a
        // healthy connection, so sessions died after 30s, or 2min, or three
        // hours, depending only on when the handset's radio was slow.
        let start = Instant::now();
        let mut heartbeat = PeerHeartbeat::new(start);
        let token = heartbeat.probe(start).expect("the first probe is sent");

        let late = start + Duration::from_secs(25);
        assert!(
            late < heartbeat.deadline(),
            "25s of quiet is not a dead phone"
        );
        // The lapsed probe is retried rather than fatal.
        let retry = heartbeat.probe(late).expect("a lapsed probe is sent again");
        assert_ne!(token, retry);

        assert!(heartbeat.acknowledge(&retry, late));
        assert_eq!(heartbeat.deadline(), late + SESSION_TIMEOUT);
    }

    #[test]
    fn only_real_silence_ends_the_session() {
        let start = Instant::now();
        let mut heartbeat = PeerHeartbeat::new(start);
        assert_eq!(heartbeat.deadline(), start + SESSION_TIMEOUT);
        let mut now = start;
        // Probing on its own never extends the session; only the phone can.
        for _ in 0..4 {
            heartbeat.probe(now);
            now += PROBE_TIMEOUT;
        }
        assert_eq!(heartbeat.deadline(), start + SESSION_TIMEOUT);
        assert!(now < heartbeat.deadline());
        assert!(start + SESSION_TIMEOUT + Duration::from_secs(1) > heartbeat.deadline());
    }

    #[test]
    fn an_application_frame_counts_as_proof_of_life() {
        // A phone that is busy sending notifications is obviously connected,
        // even if its pongs are queued behind them.
        let start = Instant::now();
        let mut heartbeat = PeerHeartbeat::new(start);
        heartbeat.probe(start);
        let busy = start + Duration::from_secs(60);
        heartbeat.mark_alive(busy);
        assert_eq!(heartbeat.deadline(), busy + SESSION_TIMEOUT);
    }

    #[test]
    fn a_pong_nobody_asked_for_proves_nothing() {
        // The probe is a challenge. If any stray pong answered it, or one for a
        // probe that lapsed long ago, it would stop measuring anything at all.
        let start = Instant::now();
        let mut heartbeat = PeerHeartbeat::new(start);
        let token = heartbeat.probe(start).expect("the first probe is sent");
        let deadline = heartbeat.deadline();
        assert!(!heartbeat.acknowledge(b"unsolicited", start));
        assert!(!heartbeat.acknowledge(&token, start + PROBE_TIMEOUT));
        assert_eq!(heartbeat.deadline(), deadline);
    }

    #[test]
    fn waking_never_spins_the_caller() {
        // `wake_at` used to be able to land on an instant at which `probe`
        // returns nothing, which turns the connection loop into a busy loop.
        let start = Instant::now();
        let mut heartbeat = PeerHeartbeat::new(start);
        let mut now = start;
        for _ in 0..6 {
            let wake = heartbeat.wake_at();
            assert!(wake >= now, "wake_at must not point into the past");
            now = wake;
            assert!(
                heartbeat.probe(now).is_some(),
                "a wake-up must always have something to send"
            );
        }
    }
}
