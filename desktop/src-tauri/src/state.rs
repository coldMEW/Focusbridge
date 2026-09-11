use crate::pairing::device_store::PairingSession;
use focusbridge_core::cert::GeneratedCert;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

const PAIRING_SESSION_SETTING: &str = "pairing.session.v1";
use tokio::sync::Notify;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDiagnostics {
    pub connected: bool,
    pub connected_at: Option<i64>,
    pub active_transport: String,
    pub last_heartbeat_at: Option<i64>,
    pub last_auth_failure: Option<String>,
    pub last_disconnect_reason: Option<String>,
}

impl Default for ConnectionDiagnostics {
    fn default() -> Self {
        Self {
            connected: false,
            connected_at: None,
            active_transport: "none".into(),
            last_heartbeat_at: None,
            last_auth_failure: None,
            last_disconnect_reason: None,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
    pub cert: Arc<GeneratedCert>,
    pairing: Arc<Mutex<Option<PairingSession>>>,
    phone_sender: Arc<Mutex<Option<UnboundedSender<String>>>>,
    diagnostics: Arc<Mutex<ConnectionDiagnostics>>,
    /// Set when the user asks for a specific phone rather than waiting for the
    /// automatic connection, and cleared once a session is established.
    relay_requested: Arc<AtomicBool>,
    relay_wake: Arc<Notify>,
    relay_idle_reason: Arc<Mutex<Option<String>>>,
    enrollment_armed: Arc<AtomicBool>,
    known_phone_allowed: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    vault_unlocked: Arc<AtomicBool>,
    known_phone_refused: Arc<AtomicBool>,
    desktop_notifications_enabled: Arc<AtomicBool>,
    /// The phone the user has connected during this run, if any.
    ///
    /// This is the backend half of a pair of features that used to be one flag
    /// (see `focusbridge_core::attach`). "Reconnect to the last phone
    /// automatically" is the user's, and it answers only "may this PC take a
    /// saved phone that turns up unasked -- at startup, or any other time?"
    /// Keeping a connection the user has already made alive across a dropped
    /// transport is this one, and it is plumbing, not a preference.
    ///
    /// Mixing them is what broke: the setting was enforced with a single-use
    /// allowance, so the phone the user had just picked was let in exactly once,
    /// and the next reattach was refused for any reason at all. A relay socket
    /// replaced, a Wi-Fi handover, a moment of packet loss -- any of them ended
    /// the session for good, on a perfectly stable connection, after thirty
    /// seconds or three hours depending only on when the first hiccup landed.
    ///
    /// Held in memory only, so a restart is still governed by the user's
    /// setting, and cleared by a manual disconnect so that stays absolute.
    approved_phone: Arc<Mutex<Option<String>>>,
    /// Set when a phone was turned away by the reconnection setting.
    ///
    /// While it holds, this PC stays where it is instead of re-dialing the
    /// relay. Re-dialing retires the pair at the relay, which force-closes the
    /// phone's socket too, so the refusal loop was actively kicking the phone
    /// off every thirty-five seconds for as long as a pairing code was on
    /// screen. Cleared the moment the user actually asks for a connection.
    awaiting_request_after_refusal: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(db_path: PathBuf, cert: GeneratedCert) -> Self {
        Self {
            db_path,
            cert: Arc::new(cert),
            pairing: Arc::new(Mutex::new(None)),
            phone_sender: Arc::new(Mutex::new(None)),
            diagnostics: Arc::new(Mutex::new(ConnectionDiagnostics::default())),
            relay_requested: Arc::new(AtomicBool::new(false)),
            relay_wake: Arc::new(Notify::new()),
            relay_idle_reason: Arc::new(Mutex::new(None)),
            enrollment_armed: Arc::new(AtomicBool::new(false)),
            known_phone_allowed: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            // Starts locked, every launch. Nothing from the phone may be put on
            // screen before someone has proved they are allowed to read it.
            vault_unlocked: Arc::new(AtomicBool::new(false)),
            known_phone_refused: Arc::new(AtomicBool::new(false)),
            // On unless the user has turned it off; startup reads the stored
            // preference over the top of this.
            desktop_notifications_enabled: Arc::new(AtomicBool::new(true)),
            approved_phone: Arc::new(Mutex::new(None)),
            awaiting_request_after_refusal: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Records that this phone is the one the user is connected to.
    ///
    /// Called once a phone has authenticated, whichever way it got in -- by
    /// scanning the code on screen, or by being picked under previous
    /// connections. From here on it may come back after a dropped transport
    /// without asking again, because the user has already said yes to it.
    pub fn approve_phone_for_this_run(&self, device_id: &str) {
        // Whatever was refused before, a phone is in now: there is nothing left
        // to stay parked for.
        self.awaiting_request_after_refusal
            .store(false, Ordering::Release);
        let mut approved = self
            .approved_phone
            .lock()
            .expect("approved phone lock poisoned");
        if approved.as_deref() != Some(device_id) {
            tracing::info!("this phone is now the connected one for this run");
            *approved = Some(device_id.to_string());
        }
    }

    /// True when this is the phone the user already connected in this run.
    ///
    /// One slot, so picking a different phone drops the first one's standing
    /// rather than leaving both able to walk in.
    pub fn is_approved_for_this_run(&self, device_id: &str) -> bool {
        self.approved_phone
            .lock()
            .expect("approved phone lock poisoned")
            .as_deref()
            == Some(device_id)
    }

    /// Forgets the connected phone. A disconnect has to mean disconnect, so
    /// nothing survives it that would let the same phone back in unasked.
    pub fn forget_approved_phone(&self) {
        *self
            .approved_phone
            .lock()
            .expect("approved phone lock poisoned") = None;
    }

    /// True while a refused phone means this PC should stay put rather than
    /// re-dial the relay into the same refusal.
    pub fn awaiting_request_after_refusal(&self) -> bool {
        self.awaiting_request_after_refusal.load(Ordering::Acquire)
    }

    /// Whether phone notifications are echoed as desktop popups.
    ///
    /// Held here rather than read from the database per notification: the
    /// answer is needed on the path a message takes from the phone to the
    /// screen, and that path should not open the encrypted database to find it.
    pub fn desktop_notifications_enabled(&self) -> bool {
        self.desktop_notifications_enabled.load(Ordering::Acquire)
    }

    pub fn set_desktop_notifications_enabled(&self, on: bool) {
        self.desktop_notifications_enabled
            .store(on, Ordering::Release);
    }

    /// Records that a phone was turned away because the user has automatic
    /// reconnection off.
    ///
    /// The relay client needs to know, because the refusal reaches it as an
    /// ordinary closed session and it reconnected straight into the same refusal
    /// -- a loop, several times a minute, for as long as the pairing screen was
    /// on display. A refusal is a decision; nothing changes until the user asks
    /// for the phone.
    pub fn note_known_phone_refused(&self) {
        self.known_phone_refused.store(true, Ordering::Release);
        // Stay put until the user asks. Re-dialing the relay retires the pair
        // there, which closes the phone's socket as well, so the loop did not
        // merely retry a refusal -- it kicked the phone off every time round.
        self.awaiting_request_after_refusal
            .store(true, Ordering::Release);
    }

    pub fn take_known_phone_refusal(&self) -> bool {
        self.known_phone_refused.swap(false, Ordering::AcqRel)
    }

    /// True once the local vault has been opened in this run.
    ///
    /// The lock used to live entirely in the interface, which hid the dashboard
    /// but did nothing about the desktop notifications the backend raises: a
    /// message arriving before the PIN was typed appeared on screen in full, and
    /// the second factor protected nothing that mattered. Anything that displays
    /// message content has to ask this first.
    pub fn vault_is_unlocked(&self) -> bool {
        self.vault_unlocked.load(Ordering::Acquire)
    }

    /// Called when the PIN or password has been verified, or a new one set.
    pub fn unlock_vault(&self) {
        self.vault_unlocked.store(true, Ordering::Release);
    }

    /// Called when the interface locks again -- the idle timeout, or signing out.
    pub fn lock_vault(&self) {
        self.vault_unlocked.store(false, Ordering::Release);
    }

    /// True after the user disconnected the phone and before they asked for it
    /// back. Disconnect has to mean disconnect: while this holds, nothing here
    /// reaches for the phone, so it cannot be pulled back by a preference or by
    /// the pairing screen simply being on display.
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    /// Cleared only by an explicit request: picking a saved phone, or asking for
    /// a fresh pairing code.
    pub fn resume(&self) {
        if self.paused.swap(false, Ordering::AcqRel) {
            tracing::info!("the pause was lifted");
        }
    }

    /// Lets a phone this PC already knows reattach once.
    ///
    /// Set when the user picks a saved phone. A phone that has just scanned the
    /// code on screen does not need it, because presenting the current pairing
    /// key is itself the request.
    pub fn allow_known_phone(&self) {
        self.known_phone_refused.store(false, Ordering::Release);
        self.known_phone_allowed.store(true, Ordering::Release);
    }

    pub fn take_known_phone_allowance(&self) -> bool {
        self.known_phone_allowed.swap(false, Ordering::AcqRel)
    }

    /// Allows the next phone to enroll, replacing any pinned identity.
    ///
    /// Armed only when the user generates a fresh pairing code, and cleared as
    /// soon as a phone takes it, so a known phone authenticates on every later
    /// connection rather than silently re-enrolling.
    pub fn arm_enrollment(&self) {
        self.enrollment_armed.store(true, Ordering::Release);
    }

    pub fn enrollment_armed(&self) -> bool {
        self.enrollment_armed.load(Ordering::Acquire)
    }

    pub fn disarm_enrollment(&self) {
        self.enrollment_armed.store(false, Ordering::Release);
    }

    /// True when this is a new reason, so it is worth saying out loud.
    pub fn note_relay_idle(&self, reason: &str) -> bool {
        let mut current = self
            .relay_idle_reason
            .lock()
            .expect("relay idle lock poisoned");
        if current.as_deref() == Some(reason) {
            return false;
        }
        *current = Some(reason.to_string());
        true
    }

    /// Asks the relay client to connect now, even when automatic connection is
    /// off. This is how "reconnect this phone" reaches a phone on another
    /// network: neither device can dial the other, so both meet at the relay.
    pub fn request_relay_connection(&self, reason: &str) {
        // Say who asked. Without this, an unexpected connection is impossible to
        // attribute, and the reconnection preference looks like it is ignored.
        tracing::info!(reason, "relay connection requested");
        // The next idle reason is worth repeating: circumstances just changed.
        *self
            .relay_idle_reason
            .lock()
            .expect("relay idle lock poisoned") = None;
        self.relay_requested.store(true, Ordering::Release);
        // The user asking is exactly the event the refusal was waiting for.
        self.awaiting_request_after_refusal
            .store(false, Ordering::Release);
        // notify_one stores a permit when nobody is parked yet. notify_waiters
        // does not, so a request arriving in the gap between the supervisor
        // testing the flag and parking was dropped, leaving it asleep forever
        // with the request still pending: pressing the button did nothing.
        self.relay_wake.notify_one();
    }

    pub fn take_relay_request(&self) -> bool {
        self.relay_requested.swap(false, Ordering::AcqRel)
    }

    pub fn relay_connection_requested(&self) -> bool {
        self.relay_requested.load(Ordering::Acquire)
    }

    /// Waits until someone asks for a connection.
    /// Waits for a request, rechecking periodically.
    ///
    /// The timeout is a safety net, not the mechanism: a permit wakes this
    /// immediately. It exists so that no future lost wakeup can strand the
    /// supervisor indefinitely, which is a bad failure because it looks exactly
    /// like the feature being broken.
    pub async fn await_relay_request(&self) {
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.relay_wake.notified(),
        )
        .await;
    }

    pub fn set_pairing(&self, session: PairingSession) {
        // Persisted as well as held in memory. A code is scanned seconds before
        // the phone finishes connecting, and if this PC restarts in that window
        // the key the phone is holding matches nothing here: no live session,
        // and no saved device either, because it never finished connecting. The
        // pairing then fails for good and rescanning cannot help, since the same
        // race can happen again.
        if let Err(error) = crate::db::store::set_setting(
            &self.db_path,
            PAIRING_SESSION_SETTING,
            &serde_json::json!({
                "deviceId": session.device_id,
                "pairingKey": session.pairing_key,
                "certFingerprint": session.cert_fingerprint,
                "expiresAt": session.expires_at,
            })
            .to_string(),
        ) {
            // Not fatal: the in-memory copy still serves this run.
            tracing::warn!(error = %error, "could not persist the pairing session");
        }
        *self.pairing.lock().expect("pairing lock poisoned") = Some(session);
    }

    /// True while the pairing screen is showing a code that has not expired.
    pub fn pairing_code_is_live(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as i64)
            .unwrap_or_default();
        self.current_pairing()
            .is_some_and(|session| session.expires_at > now)
    }

    pub fn current_pairing(&self) -> Option<PairingSession> {
        let mut held = self.pairing.lock().expect("pairing lock poisoned");
        if held.is_none() {
            // First read after a restart: recover the code that is still valid.
            *held = self.load_pairing_session();
        }
        held.clone()
    }

    /// Forgets the pairing code on screen, in memory and on disk.
    pub fn clear_pairing_session(&self) {
        *self.pairing.lock().expect("pairing lock poisoned") = None;
        if let Err(error) =
            crate::db::store::set_setting(&self.db_path, PAIRING_SESSION_SETTING, "")
        {
            tracing::warn!(error = %error, "could not clear the stored pairing session");
        }
    }

    fn load_pairing_session(&self) -> Option<PairingSession> {
        let stored =
            crate::db::store::get_setting(&self.db_path, PAIRING_SESSION_SETTING).ok()??;
        if stored.trim().is_empty() {
            return None;
        }
        let value: serde_json::Value = serde_json::from_str(&stored).ok()?;
        let text = |key: &str| value.get(key)?.as_str().map(str::to_string);
        let session = PairingSession {
            device_id: text("deviceId")?,
            pairing_key: text("pairingKey")?,
            cert_fingerprint: text("certFingerprint")?,
            expires_at: value.get("expiresAt")?.as_i64()?,
        };
        // An expired code is not a pairing offer; let it go rather than reviving it.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as i64)
            .unwrap_or_default();
        (session.expires_at > now).then_some(session)
    }

    pub fn set_phone_sender(&self, sender: UnboundedSender<String>) {
        *self
            .phone_sender
            .lock()
            .expect("phone sender lock poisoned") = Some(sender);
        self.update_diagnostics(|diag| {
            diag.connected = true;
            diag.connected_at = Some(now_ms_i64());
            diag.last_heartbeat_at = None;
            diag.last_auth_failure = None;
            diag.last_disconnect_reason = None;
        });
    }

    pub fn clear_phone_sender(&self) {
        *self
            .phone_sender
            .lock()
            .expect("phone sender lock poisoned") = None;
    }

    pub fn is_current_phone_sender(&self, sender: &UnboundedSender<String>) -> bool {
        self.phone_sender
            .lock()
            .expect("phone sender lock poisoned")
            .as_ref()
            .map(|active| active.same_channel(sender))
            .unwrap_or(false)
    }

    pub fn mark_manual_disconnect(&self) {
        tracing::info!("the user disconnected the phone here");
        self.paused.store(true, Ordering::Release);
        // Retire the code that is on screen.
        //
        // A phone keeps the pairing key it was given, and the pairing session
        // lives for five minutes, so for those five minutes the phone that was
        // just disconnected still presents the key of the code on display -- and
        // was let straight back in as though it had just scanned it. The
        // disconnect held for about seven seconds.
        //
        // Retiring the session makes the next code a genuinely new one, so
        // "presents the code on screen" once again means what it says: a phone
        // that has scanned since the disconnect.
        self.clear_pairing_session();
        // A pending allowance would let the phone straight back in.
        self.known_phone_allowed.store(false, Ordering::Release);
        // So would its standing as the phone connected in this run. Surviving a
        // disconnect is precisely what rule two forbids.
        self.forget_approved_phone();
        self.relay_requested.store(false, Ordering::Release);
        self.clear_phone_sender();
        self.update_diagnostics(|diag| {
            diag.connected = false;
            diag.connected_at = None;
            diag.active_transport = "none".into();
            diag.last_disconnect_reason = Some("manual disconnect".into());
        });
    }

    pub fn clear_phone_sender_if_current(&self, sender: &UnboundedSender<String>) -> bool {
        self.clear_phone_sender_if_current_with_reason(sender, "socket closed")
    }

    pub fn clear_phone_sender_if_current_with_reason(
        &self,
        sender: &UnboundedSender<String>,
        reason: &str,
    ) -> bool {
        let mut current = self
            .phone_sender
            .lock()
            .expect("phone sender lock poisoned");
        if current
            .as_ref()
            .map(|active| active.same_channel(sender))
            .unwrap_or(false)
        {
            *current = None;
            // Say why, out loud. The reason was recorded for the diagnostics
            // panel and nowhere else, so "it disconnected on its own" could only
            // be answered by catching the app with the panel open. It is the
            // first thing anyone needs when a session ends by itself.
            tracing::warn!(reason, "the phone session ended");
            self.update_diagnostics(|diag| {
                diag.connected = false;
                diag.connected_at = None;
                diag.active_transport = "none".into();
                diag.last_disconnect_reason = Some(reason.into());
            });
            return true;
        }
        false
    }

    pub fn send_to_phone(&self, message: String) -> bool {
        self.phone_sender
            .lock()
            .expect("phone sender lock poisoned")
            .as_ref()
            .map(|sender| sender.send(message).is_ok())
            .unwrap_or(false)
    }

    pub fn mark_transport(&self, transport: &str) {
        self.update_diagnostics(|diag| {
            diag.active_transport = transport.to_string();
            diag.last_disconnect_reason = None;
        });
    }

    pub fn mark_heartbeat(&self, at: i64) {
        self.update_diagnostics(|diag| {
            diag.last_heartbeat_at = Some(at);
        });
    }

    pub fn mark_auth_failed(&self, reason: &str) {
        self.update_diagnostics(|diag| {
            diag.connected = false;
            diag.connected_at = None;
            diag.last_auth_failure = Some(reason.to_string());
            diag.last_disconnect_reason = Some("authentication failed".into());
        });
    }

    pub fn mark_stale_connection(&self, reason: &str) {
        self.update_diagnostics(|diag| {
            diag.connected = false;
            diag.connected_at = None;
            diag.active_transport = "none".into();
            diag.last_disconnect_reason = Some(reason.to_string());
        });
    }

    pub fn diagnostics(&self) -> ConnectionDiagnostics {
        self.diagnostics
            .lock()
            .expect("diagnostics lock poisoned")
            .clone()
    }

    fn update_diagnostics(&self, update: impl FnOnce(&mut ConnectionDiagnostics)) {
        let mut diagnostics = self.diagnostics.lock().expect("diagnostics lock poisoned");
        update(&mut diagnostics);
    }
}

fn now_ms_i64() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod vault_lock_tests {
    use super::*;

    fn state() -> AppState {
        let dir = std::env::temp_dir().join(format!("fb-vault-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Built straight from the core crate: this file is also compiled into an
        // integration test, where the desktop module tree is not in scope.
        let cert = focusbridge_core::cert::generate_self_signed("focusbridge-test")
            .expect("generate a certificate for the test");
        AppState::new(dir.join("test.db"), cert)
    }

    #[test]
    fn a_fresh_launch_starts_locked() {
        // The whole point of the second factor: a message arriving before anyone
        // has typed the PIN must not be put on screen.
        assert!(!state().vault_is_unlocked());
    }

    #[test]
    fn disconnecting_retires_the_pairing_code_on_screen() {
        // Without this the phone that was just disconnected still holds the key
        // of the code on display, is read as having scanned it, and reconnects
        // within seconds. The disconnect lasted about seven seconds.
        let state = state();
        state.set_pairing(PairingSession {
            device_id: "desktop".into(),
            pairing_key: "a".repeat(64),
            cert_fingerprint: "b".repeat(64),
            expires_at: i64::MAX,
        });
        assert!(state.pairing_code_is_live());

        state.mark_manual_disconnect();

        assert!(state.is_paused());
        assert!(
            !state.pairing_code_is_live(),
            "the disconnected phone can still present the live code"
        );
        assert!(state.current_pairing().is_none());
    }

    #[test]
    fn the_vault_opens_on_unlock_and_closes_again_on_lock() {
        let state = state();
        state.unlock_vault();
        assert!(state.vault_is_unlocked());
        // The idle timeout and signing out both come back through here, so a
        // desktop left alone stops showing messages again.
        state.lock_vault();
        assert!(!state.vault_is_unlocked());
    }
}

#[cfg(test)]
mod connected_phone_tests {
    use super::*;

    fn state() -> AppState {
        let dir = std::env::temp_dir().join(format!("fb-connected-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cert = focusbridge_core::cert::generate_self_signed("focusbridge-test")
            .expect("generate a certificate for the test");
        AppState::new(dir.join("test.db"), cert)
    }

    #[test]
    fn a_launch_knows_no_connected_phone() {
        // The reconnection setting still governs starting a connection. Carrying
        // the standing across a restart would be the silent reattach on launch
        // that this project has shipped broken twice.
        assert!(!state().is_approved_for_this_run("phone-a"));
    }

    #[test]
    fn the_phone_that_connected_may_come_back() {
        // The bug: the setting was enforced per socket, so the phone the user had
        // just connected was refused the moment the transport under it dropped.
        let state = state();
        state.approve_phone_for_this_run("phone-a");
        assert!(state.is_approved_for_this_run("phone-a"));
    }

    #[test]
    fn only_that_phone_may_come_back() {
        let state = state();
        state.approve_phone_for_this_run("phone-a");
        assert!(!state.is_approved_for_this_run("phone-b"));
        // One slot: connecting a second phone ends the first one's standing
        // rather than leaving both able to walk in.
        state.approve_phone_for_this_run("phone-b");
        assert!(!state.is_approved_for_this_run("phone-a"));
        assert!(state.is_approved_for_this_run("phone-b"));
    }

    #[test]
    fn a_manual_disconnect_ends_the_standing() {
        // Rule two. The phone the user was connected to is exactly the phone a
        // disconnect is aimed at, so this must not outlive it.
        let state = state();
        state.approve_phone_for_this_run("phone-a");
        state.mark_manual_disconnect();
        assert!(!state.is_approved_for_this_run("phone-a"));
    }

    #[test]
    fn a_refusal_parks_this_pc_until_the_user_asks() {
        // Re-dialing reached the same refusal, and rejoining the relay retires
        // the pair there, which closes the phone's socket too -- so the loop
        // kicked the phone off every thirty-five seconds.
        let state = state();
        assert!(!state.awaiting_request_after_refusal());
        state.note_known_phone_refused();
        assert!(state.awaiting_request_after_refusal());
        state.request_relay_connection("the user asked to reconnect a saved phone");
        assert!(!state.awaiting_request_after_refusal());
    }

    #[test]
    fn the_safety_net_timeout_is_not_the_user_asking() {
        // `await_relay_request` returns on a timeout as well as on a request, and
        // treating that return as permission to dial is what made the loop.
        let state = state();
        state.note_known_phone_refused();
        assert!(state.awaiting_request_after_refusal());
        assert!(
            !state.relay_connection_requested(),
            "nothing has asked for a connection"
        );
        assert!(
            state.awaiting_request_after_refusal(),
            "only the user asking may lift this"
        );
    }

    #[test]
    fn a_phone_getting_in_clears_the_parking() {
        let state = state();
        state.note_known_phone_refused();
        state.approve_phone_for_this_run("phone-a");
        assert!(!state.awaiting_request_after_refusal());
    }
}
