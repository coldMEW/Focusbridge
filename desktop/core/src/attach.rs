//! Which phone this PC will let attach.
//!
//! # Two features, deliberately separate
//!
//! These were one flag once, and mixing them is what made a stable connection
//! drop. They are different things, owned by different people, and must not be
//! folded back together:
//!
//! 1. **The first connection — the user's, and only the user's.** "Reconnect to
//!    the last phone automatically" answers one question: when FocusBridge
//!    starts, or when a saved phone turns up that nobody asked for, may this PC
//!    take it? Off means this PC does not reach out on its own; the user picks a
//!    phone under previous connections, or shows a code. That is a preference,
//!    and it is `auto_first_connection` here.
//!
//! 2. **Keeping a live connection alive — the backend's, always.** Once the user
//!    has connected a phone, the transport underneath will occasionally drop:
//!    a relay socket is replaced, Wi-Fi hands over, a packet is lost. Putting
//!    that back is plumbing. The user already said yes to this phone; they are
//!    not being asked to say yes again every time a radio blinks. That is
//!    `resuming_the_users_connection` here, and it is not a setting.
//!
//! Enforcing (1) on every socket meant it silently did (2)'s job as well, and did
//! it wrong: a single-use allowance was spent on the first attach, so the phone
//! was refused the moment anything underneath it flinched. The connection then
//! died after thirty seconds, or two minutes, or three hours -- not on a timer,
//! but on whenever the first hiccup happened to land.
//!
//! # Why this lives in the core crate
//!
//! Because it is the rule this project has broken most often, and in the desktop
//! crate it was tested by nothing: that crate sets `[lib] test = false`, and
//! `ws_server.rs` is not pulled into any integration test, so `mod attach_tests`
//! was compiled by nothing and run by nothing -- while the behaviour checklist
//! named it as the guarantee. Three regressions of this rule reached the user.
//! Here the tests actually run.

/// Whether this phone may attach now.
///
/// - `paused` — the user pressed Disconnect. This outranks everything below.
/// - `auto_first_connection` — the user's "reconnect to the last phone
///   automatically" setting. Governs starting a connection, nothing else.
/// - `holds_the_code_on_screen` — this phone is presenting the pairing code
///   currently displayed, which is the user asking for it in person.
/// - `resuming_the_users_connection` — this is the phone the user already
///   connected during this run, coming back after its transport dropped. The
///   backend's job, independent of the setting.
/// - `take_allowance` — the single-use permission granted by picking a phone by
///   name. Consulted last, so it is never spent on a connection that was already
///   allowed.
pub fn may_attach(
    paused: bool,
    auto_first_connection: bool,
    holds_the_code_on_screen: bool,
    resuming_the_users_connection: bool,
    take_allowance: impl FnOnce() -> bool,
) -> bool {
    if paused {
        // A manual disconnect outranks both features, including having been the
        // connected phone a moment ago -- that is the whole of rule two. The
        // standing is dropped at the disconnect as well; this is the second lock
        // on the same door.
        return holds_the_code_on_screen || take_allowance();
    }
    auto_first_connection
        || holds_the_code_on_screen
        || resuming_the_users_connection
        || take_allowance()
}

#[cfg(test)]
mod attach_tests {
    use super::may_attach;
    use std::cell::Cell;

    /// A phone arriving on its own, that the user has not connected in this run.
    /// Feature 1 decides this one.
    const A_FIRST_CONNECTION: bool = false;
    /// The phone the user is already connected to, coming back after the
    /// transport dropped. Feature 2 decides this one.
    const RESUMING: bool = true;

    #[test]
    fn a_phone_that_just_scanned_the_code_always_attaches() {
        assert!(may_attach(false, false, true, A_FIRST_CONNECTION, || false));
        // Even from a PC the user had disconnected: scanning is asking for it.
        assert!(may_attach(true, false, true, A_FIRST_CONNECTION, || false));
    }

    #[test]
    fn a_disconnected_pc_accepts_nothing_by_itself() {
        // Disconnect means disconnect. This held over the relay and not over the
        // local network, so with automatic reconnection on, a phone reattached
        // over Wi-Fi seconds after the user pressed Disconnect.
        assert!(!may_attach(true, true, false, A_FIRST_CONNECTION, || false));
        assert!(!may_attach(true, false, false, A_FIRST_CONNECTION, || {
            false
        }));
        // Unless the user asked for that phone by name.
        assert!(may_attach(true, false, false, A_FIRST_CONNECTION, || true));
    }

    #[test]
    fn a_disconnect_outranks_resuming() {
        // Feature 2 must not undo rule two. The phone the user was talking to a
        // second ago is exactly the phone a disconnect is aimed at.
        assert!(!may_attach(true, false, false, RESUMING, || false));
        assert!(!may_attach(true, true, false, RESUMING, || false));
    }

    // ---- Feature 1: the first connection, the user's setting ----

    #[test]
    fn the_setting_off_refuses_a_saved_phone_that_arrives_unasked() {
        // The bug the user hit twice: this held over the relay and not over the
        // local network, so the desktop reattached to the last phone on launch
        // with the setting plainly turned off.
        assert!(!may_attach(false, false, false, A_FIRST_CONNECTION, || {
            false
        }));
    }

    #[test]
    fn the_setting_on_lets_a_saved_phone_straight_in() {
        assert!(may_attach(false, true, false, A_FIRST_CONNECTION, || false));
    }

    #[test]
    fn a_saved_phone_attaches_when_the_user_asked_for_it_by_name() {
        assert!(may_attach(false, false, false, A_FIRST_CONNECTION, || true));
    }

    // ---- Feature 2: keeping the user's connection alive, always ----

    #[test]
    fn resuming_works_with_the_setting_off() {
        // The whole point of separating them. The setting is about starting a
        // connection; it has no opinion about one the user is already in.
        assert!(may_attach(false, false, false, RESUMING, || false));
    }

    #[test]
    fn resuming_does_not_need_the_setting_on_or_a_code_on_screen() {
        // Neither of the other two doors has to be open for the backend to put a
        // dropped transport back.
        assert!(may_attach(false, false, false, RESUMING, || false));
    }

    #[test]
    fn resuming_never_spends_the_allowance() {
        // The allowance is single-use and belongs to the *next* phone the user
        // asks for by name. Burning it on a reattach that was already permitted
        // would refuse that one.
        let taken = Cell::new(false);
        assert!(may_attach(false, false, false, RESUMING, || {
            taken.set(true);
            true
        }));
        assert!(!taken.get());
    }

    #[test]
    fn only_the_connected_phone_resumes_a_different_one_still_meets_the_setting() {
        // Feature 2 is not a way around feature 1. Another handset has not been
        // connected in this run, so the setting decides it, exactly as before.
        assert!(!may_attach(false, false, false, A_FIRST_CONNECTION, || {
            false
        }));
    }

    #[test]
    fn the_allowance_is_not_spent_unless_it_is_needed() {
        // It is a single-use permission; consuming it on a connection that was
        // already allowed would silently refuse the next one.
        let taken = Cell::new(false);
        assert!(may_attach(false, true, false, A_FIRST_CONNECTION, || {
            taken.set(true);
            true
        }));
        assert!(!taken.get());
        assert!(may_attach(false, false, true, A_FIRST_CONNECTION, || {
            taken.set(true);
            true
        }));
        assert!(!taken.get());
    }
}
