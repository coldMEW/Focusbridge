use super::*;

fn ready() -> (Session, Session) {
    let phone_id = Identity::generate().unwrap();
    let desktop_id = Identity::generate().unwrap();
    let mut phone = Session::phone(&phone_id, &[1; 32], [2; 16], desktop_id.public_key()).unwrap();
    let mut desktop =
        Session::desktop(&desktop_id, &[1; 32], [2; 16], phone_id.public_key()).unwrap();
    desktop
        .read_handshake(&phone.write_handshake().unwrap())
        .unwrap();
    phone
        .read_handshake(&desktop.write_handshake().unwrap())
        .unwrap();
    desktop
        .read_handshake(&phone.write_handshake().unwrap())
        .unwrap();
    phone
        .read_confirmation(&desktop.write_confirmation().unwrap())
        .unwrap();
    desktop
        .read_confirmation(&phone.write_confirmation().unwrap())
        .unwrap();
    (phone, desktop)
}

fn assert_destroyed(session: &Session) {
    assert!(session.is_closed());
    assert!(session.transport.is_none());
    assert!(session.handshake.is_none());
    assert!(session.binding_hash().is_none());
    assert!(session.peer_identity().is_none());
}

#[test]
fn idle_handshake_expires_without_needing_an_incoming_frame() {
    let mut session =
        Session::desktop_enrollment(&Identity::generate().unwrap(), &[1; 32], [2; 16]).unwrap();
    session.created = Instant::now() - HANDSHAKE_AGE;
    assert_eq!(session.check_alive(), Err(Error::LimitReached));
    assert_destroyed(&session);
}

#[test]
fn established_session_expires_and_requires_new_keys() {
    let (mut phone, _) = ready();
    assert_eq!(
        phone.checked_at(phone.created + MAX_AGE, |_| Ok(())),
        Err(Error::LimitReached)
    );
    assert_destroyed(&phone);
}

#[test]
fn outgoing_message_budget_rejects_before_nonce_use() {
    let (mut phone, _) = ready();
    phone.sent_messages = MAX_MESSAGES;
    assert_eq!(phone.seal_record(b"not sent"), Err(Error::LimitReached));
    assert_eq!(phone.sent_messages, MAX_MESSAGES);
    assert_destroyed(&phone);
}

#[test]
fn incoming_message_budget_rejects_before_dispatch() {
    let (mut phone, mut desktop) = ready();
    desktop.received_messages = MAX_MESSAGES;
    let frame = phone.seal_record(b"not dispatched").unwrap().remove(0);
    assert_eq!(desktop.open_frame(&frame), Err(Error::LimitReached));
    assert_eq!(desktop.received_messages, MAX_MESSAGES);
    assert_destroyed(&desktop);
}

#[test]
fn record_that_exhausts_budget_mid_chunk_returns_no_ciphertext() {
    let (mut phone, _) = ready();
    phone.sent_messages = MAX_MESSAGES - 1;
    assert_eq!(
        phone.seal_record(&vec![9; records::CHUNK + 1]),
        Err(Error::LimitReached)
    );
    assert_destroyed(&phone);
}

#[test]
fn outgoing_byte_budget_counts_authenticated_record_header() {
    let (mut phone, _) = ready();
    phone.sent_bytes = MAX_BYTES - records::HEADER as u64;
    assert_eq!(phone.seal_record(b"x"), Err(Error::LimitReached));
    assert_destroyed(&phone);
}

#[test]
fn incoming_byte_budget_counts_authenticated_record_header() {
    let (mut phone, mut desktop) = ready();
    desktop.received_bytes = MAX_BYTES - records::HEADER as u64;
    let frame = phone.seal_record(b"x").unwrap().remove(0);
    assert_eq!(desktop.open_frame(&frame), Err(Error::LimitReached));
    assert_destroyed(&desktop);
}

#[test]
fn authenticated_but_malformed_record_header_closes_session() {
    let (mut phone, mut desktop) = ready();
    let mut malformed = vec![0; records::HEADER];
    malformed[8..12].copy_from_slice(&1u32.to_be_bytes());
    malformed[12..16].copy_from_slice(&1u32.to_be_bytes());
    malformed.push(3);
    let ciphertext = phone.encrypt(&malformed).unwrap();
    assert_eq!(desktop.open_frame(&ciphertext), Err(Error::InvalidInput));
    assert_destroyed(&desktop);
}
