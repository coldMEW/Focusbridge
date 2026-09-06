use super::*;
use crate::Error;
use focusbridge_secure_channel::{Identity, Phase};
use std::sync::{Arc, Mutex};

fn session() -> Result<Session> {
    let identity = Identity::from_private([7; 32])?;
    Ok(Session::phone(&identity, &[9; 32], [1; 16], [2; 32])?)
}

#[test]
fn count_is_bounded_before_construction() {
    let mut registry = Registry::new();
    for _ in 0..MAX_SESSIONS {
        registry.insert_with(session).unwrap();
    }
    assert_eq!(
        registry.insert_with(|| panic!("must not construct")),
        Err(Error::Capacity)
    );
    registry.close(1);
    assert!(registry.insert_with(session).unwrap() > MAX_SESSIONS as i64);
}

#[test]
fn ids_never_reuse_and_close_is_idempotent() {
    let mut registry = Registry::new();
    let first = registry.insert_with(session).unwrap();
    registry.close(first);
    registry.close(first);
    registry.close(0);
    registry.close(-1);
    let second = registry.insert_with(session).unwrap();
    assert!(first > 0 && second > first);
    for handle in [first, 0, -1, i64::MAX] {
        assert_eq!(
            registry.with_session(handle, |_| Ok(())),
            Err(Error::InvalidHandle)
        );
    }
    assert_eq!(registry.sessions.len(), 1);
}

#[test]
fn exhaustion_never_wraps_or_restarts_ids() {
    let mut registry = Registry::new();
    registry.next = i64::MAX;
    let last = registry.insert_with(session).unwrap();
    assert_eq!(last, i64::MAX);
    registry.close(last);
    assert_eq!(
        registry.insert_with(|| panic!("must not construct")),
        Err(Error::Capacity)
    );
    registry.close_all();
    assert_eq!(registry.insert_with(session), Err(Error::Capacity));
}

#[test]
fn constructor_failure_does_not_take_capacity() {
    let mut registry = Registry::new();
    assert_eq!(
        registry.insert_with(|| Err(Error::InvalidInput)),
        Err(Error::InvalidInput)
    );
    assert!(registry.sessions.is_empty());
    assert!(registry.insert_with(session).is_ok());
}

#[test]
fn engine_error_retires_handle_without_harming_other_sessions() {
    let mut registry = Registry::new();
    let failed = registry.insert_with(session).unwrap();
    let other = registry.insert_with(session).unwrap();
    assert!(registry
        .with_session(failed, |s| Ok(s.seal_record(b"not ready")?))
        .is_err());
    assert_eq!(
        registry.with_session(failed, |_| Ok(())),
        Err(Error::InvalidHandle)
    );
    assert_eq!(
        registry.with_session(other, |s| Ok(s.phase())),
        Ok(Phase::Handshake)
    );
}

#[test]
fn boundary_input_or_output_error_retires_handle() {
    let mut registry = Registry::new();
    for error in [Error::InvalidInput, Error::Jni] {
        let handle = registry.insert_with(session).unwrap();
        assert!(registry.with_session::<()>(handle, |_| Err(error)).is_err());
        assert!(!registry.sessions.contains_key(&handle));
    }
}

#[test]
fn panic_retires_session_and_does_not_poison_lock() {
    let registry = Mutex::new(Registry::new());
    let handle = registry.lock().unwrap().insert_with(session).unwrap();
    assert_eq!(
        registry
            .lock()
            .unwrap()
            .with_session::<()>(handle, |_| panic!("synthetic failure")),
        Err(Error::Internal)
    );
    assert!(registry.lock().unwrap().sessions.is_empty());
}

#[test]
fn cleanup_removes_every_session_but_not_id_history() {
    let mut registry = Registry::new();
    let first = registry.insert_with(session).unwrap();
    registry.insert_with(session).unwrap();
    registry.close_all();
    assert!(registry.sessions.is_empty());
    assert!(registry.insert_with(session).unwrap() > first + 1);
}

#[test]
fn operations_and_close_share_the_same_serialization_lock() {
    let registry = Arc::new(Mutex::new(Registry::new()));
    let handle = registry.lock().unwrap().insert_with(session).unwrap();
    let inside = Arc::clone(&registry);
    registry
        .lock()
        .unwrap()
        .with_session(handle, |_| {
            std::thread::scope(|scope| {
                scope.spawn(|| assert!(inside.try_lock().is_err()));
            });
            Ok(())
        })
        .unwrap();
    registry.lock().unwrap().close(handle);
    assert!(registry
        .lock()
        .unwrap()
        .with_session(handle, |_| Ok(()))
        .is_err());
}
