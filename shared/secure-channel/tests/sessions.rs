use focusbridge_secure_channel::{Identity, Session};

const PAIR: [u8; 16] = [0x31; 16];
const PSK: [u8; 32] = [0x72; 32];

fn handshake(phone: &mut Session, desktop: &mut Session) {
    desktop
        .read_handshake(&phone.write_handshake().unwrap())
        .unwrap();
    phone
        .read_handshake(&desktop.write_handshake().unwrap())
        .unwrap();
    desktop
        .read_handshake(&phone.write_handshake().unwrap())
        .unwrap();
}

fn ready(phone_id: &Identity, desktop_id: &Identity) -> (Session, Session) {
    let mut phone = Session::phone(phone_id, &PSK, PAIR, desktop_id.public_key()).unwrap();
    let mut desktop = Session::desktop(desktop_id, &PSK, PAIR, phone_id.public_key()).unwrap();
    handshake(&mut phone, &mut desktop);
    assert!(!phone.is_ready());
    assert!(!desktop.is_ready());
    phone
        .read_confirmation(&desktop.write_confirmation().unwrap())
        .unwrap();
    desktop
        .read_confirmation(&phone.write_confirmation().unwrap())
        .unwrap();
    assert!(phone.is_ready() && desktop.is_ready());
    (phone, desktop)
}

#[test]
fn pinned_peers_exchange_an_encrypted_record_in_each_direction() {
    let phone_id = Identity::generate().unwrap();
    let desktop_id = Identity::generate().unwrap();
    let (mut phone, mut desktop) = ready(&phone_id, &desktop_id);
    let plaintext = b"private notification fixture";
    let frames = phone.seal_record(plaintext).unwrap();
    assert_eq!(frames.len(), 1);
    assert!(!frames[0]
        .windows(plaintext.len())
        .any(|part| part == plaintext));
    assert_eq!(
        desktop.open_frame(&frames[0]).unwrap().unwrap().as_slice(),
        plaintext
    );
    let ack = desktop.seal_record(b"application ACK").unwrap();
    assert_eq!(
        phone.open_frame(&ack[0]).unwrap().unwrap().as_slice(),
        b"application ACK"
    );
}

#[test]
fn replay_kills_the_session_and_cannot_be_retried() {
    let (mut phone, mut desktop) = ready(
        &Identity::generate().unwrap(),
        &Identity::generate().unwrap(),
    );
    let frame = phone.seal_record(b"once").unwrap().remove(0);
    desktop.open_frame(&frame).unwrap();
    assert!(desktop.open_frame(&frame).is_err());
    assert!(desktop.is_closed());
    assert!(desktop
        .open_frame(&phone.seal_record(b"later").unwrap()[0])
        .is_err());
}

#[test]
fn initial_enrollment_requires_explicit_phone_identity_approval() {
    let phone_id = Identity::generate().unwrap();
    let desktop_id = Identity::generate().unwrap();
    let mut phone = Session::phone(&phone_id, &PSK, PAIR, desktop_id.public_key()).unwrap();
    let mut desktop = Session::desktop_enrollment(&desktop_id, &PSK, PAIR).unwrap();
    handshake(&mut phone, &mut desktop);
    assert!(!phone.is_ready() && !desktop.is_ready());
    desktop
        .approve_enrollment(phone_id.public_key(), desktop.binding_hash().unwrap())
        .unwrap();
    phone
        .read_confirmation(&desktop.write_confirmation().unwrap())
        .unwrap();
    desktop
        .read_confirmation(&phone.write_confirmation().unwrap())
        .unwrap();
    assert!(phone.is_ready() && desktop.is_ready());
}

#[test]
fn old_session_ciphertext_fails_after_reconnect() {
    let phone_id = Identity::generate().unwrap();
    let desktop_id = Identity::generate().unwrap();
    let (mut old_phone, _) = ready(&phone_id, &desktop_id);
    let old = old_phone.seal_record(b"old session").unwrap().remove(0);
    let (_, mut new_desktop) = ready(&phone_id, &desktop_id);
    assert!(new_desktop.open_frame(&old).is_err());
    assert!(new_desktop.is_closed());
}

#[test]
fn wrong_desktop_identity_is_rejected_before_phone_final_handshake() {
    let phone_id = Identity::generate().unwrap();
    let desktop_id = Identity::generate().unwrap();
    let wrong_id = Identity::generate().unwrap();
    let mut phone = Session::phone(&phone_id, &PSK, PAIR, wrong_id.public_key()).unwrap();
    let mut desktop = Session::desktop(&desktop_id, &PSK, PAIR, phone_id.public_key()).unwrap();
    desktop
        .read_handshake(&phone.write_handshake().unwrap())
        .unwrap();
    assert!(phone
        .read_handshake(&desktop.write_handshake().unwrap())
        .is_err());
    assert!(phone.is_closed());
    assert!(phone.write_handshake().is_err());
}

#[test]
fn large_inventory_uses_bounded_authenticated_chunks() {
    let (mut phone, mut desktop) = ready(
        &Identity::generate().unwrap(),
        &Identity::generate().unwrap(),
    );
    let record = vec![0x61; 1024 * 1024];
    let frames = phone.seal_record(&record).unwrap();
    assert!(frames.len() > 1);
    let mut decoded = None;
    for (index, frame) in frames.iter().enumerate() {
        assert!(frame.len() <= 65535);
        let current = desktop.open_frame(frame).unwrap();
        if index < frames.len() - 1 {
            assert!(current.is_none());
        }
        decoded = current;
    }
    assert_eq!(decoded.unwrap().as_slice(), record);
}

#[test]
fn restored_device_identity_preserves_its_pin() {
    let identity = Identity::generate().unwrap();
    assert_eq!(
        Identity::from_private(*identity.export_private())
            .unwrap()
            .public_key(),
        identity.public_key()
    );
    assert!(Identity::from_private([0; 32]).is_err());
}

#[test]
fn wrong_psk_fails_without_ready_or_application_dispatch() {
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    let mut phone = Session::phone(&a, &[0x73; 32], PAIR, b.public_key()).unwrap();
    let mut desktop = Session::desktop(&b, &PSK, PAIR, a.public_key()).unwrap();
    desktop
        .read_handshake(&phone.write_handshake().unwrap())
        .unwrap();
    phone
        .read_handshake(&desktop.write_handshake().unwrap())
        .unwrap();
    assert!(desktop
        .read_handshake(&phone.write_handshake().unwrap())
        .is_err());
    assert!(desktop.is_closed());
    assert!(!phone.is_ready());
}

#[test]
fn wrong_saved_phone_identity_does_not_reopen_enrollment() {
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    let mut phone = Session::phone(&a, &PSK, PAIR, b.public_key()).unwrap();
    let mut desktop =
        Session::desktop(&b, &PSK, PAIR, Identity::generate().unwrap().public_key()).unwrap();
    desktop
        .read_handshake(&phone.write_handshake().unwrap())
        .unwrap();
    phone
        .read_handshake(&desktop.write_handshake().unwrap())
        .unwrap();
    assert!(desktop
        .read_handshake(&phone.write_handshake().unwrap())
        .is_err());
    assert!(desktop.is_closed());
    assert!(desktop.approve_enrollment(a.public_key(), [0; 32]).is_err());
}

#[test]
fn different_pair_context_rejects_the_handshake() {
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    let mut phone = Session::phone(&a, &PSK, PAIR, b.public_key()).unwrap();
    let mut desktop = Session::desktop(&b, &PSK, [0x32; 16], a.public_key()).unwrap();
    let result = desktop
        .read_handshake(&phone.write_handshake().unwrap())
        .and_then(|_| phone.read_handshake(&desktop.write_handshake().unwrap()))
        .and_then(|_| desktop.read_handshake(&phone.write_handshake().unwrap()));
    assert!(result.is_err());
    assert!(!phone.is_ready() && !desktop.is_ready());
}

#[test]
fn application_traffic_before_confirmation_fails_closed() {
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    let mut phone = Session::phone(&a, &PSK, PAIR, b.public_key()).unwrap();
    assert!(phone.seal_record(b"too early").is_err());
    assert!(phone.is_closed());
    let mut phone = Session::phone(&a, &PSK, PAIR, b.public_key()).unwrap();
    let mut desktop = Session::desktop(&b, &PSK, PAIR, a.public_key()).unwrap();
    handshake(&mut phone, &mut desktop);
    assert!(phone
        .seal_record(b"handshake alone is not readiness")
        .is_err());
    assert!(phone.is_closed());
}

#[test]
fn enrollment_cannot_confirm_without_local_approval() {
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    let mut phone = Session::phone(&a, &PSK, PAIR, b.public_key()).unwrap();
    let mut desktop = Session::desktop_enrollment(&b, &PSK, PAIR).unwrap();
    handshake(&mut phone, &mut desktop);
    assert!(desktop.write_confirmation().is_err());
    assert!(desktop.is_closed());
}

#[test]
fn approval_from_another_handshake_is_rejected() {
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    let (previous, _) = ready(&a, &b);
    let mut phone = Session::phone(&a, &PSK, PAIR, b.public_key()).unwrap();
    let mut desktop = Session::desktop_enrollment(&b, &PSK, PAIR).unwrap();
    handshake(&mut phone, &mut desktop);
    assert!(desktop
        .approve_enrollment(a.public_key(), previous.binding_hash().unwrap())
        .is_err());
    assert!(desktop.is_closed());
}

#[test]
fn reflected_confirmation_does_not_establish_ready() {
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    let mut phone = Session::phone(&a, &PSK, PAIR, b.public_key()).unwrap();
    let mut desktop = Session::desktop(&b, &PSK, PAIR, a.public_key()).unwrap();
    handshake(&mut phone, &mut desktop);
    let reflected = desktop.write_confirmation().unwrap();
    assert!(desktop.read_confirmation(&reflected).is_err());
    assert!(desktop.is_closed());
}

#[test]
fn ciphertext_reflection_and_bit_tampering_fail_closed() {
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    let (mut phone, _) = ready(&a, &b);
    let reflected = phone.seal_record(b"never reflect").unwrap().remove(0);
    assert!(phone.open_frame(&reflected).is_err());
    assert!(phone.is_closed());
    for bit in [0, 8, 16, 32] {
        let (mut phone, mut desktop) = ready(&a, &b);
        let mut changed = phone.seal_record(b"never modify").unwrap().remove(0);
        changed[bit] ^= 0x80;
        assert!(desktop.open_frame(&changed).is_err());
        assert!(desktop.is_closed());
    }
}

#[test]
fn truncated_and_reordered_chunks_never_release_a_prefix() {
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    let (mut phone, mut desktop) = ready(&a, &b);
    let frames = phone.seal_record(&vec![0x42; 100_000]).unwrap();
    assert!(desktop.open_frame(&frames[1]).is_err());
    assert!(desktop.is_closed());
    let (mut phone, mut desktop) = ready(&a, &b);
    let mut frames = phone.seal_record(&vec![0x42; 100_000]).unwrap();
    assert!(desktop.open_frame(&frames[0]).unwrap().is_none());
    frames[1].pop();
    assert!(desktop.open_frame(&frames[1]).is_err());
    assert!(desktop.is_closed());
}

#[test]
fn configuration_and_frame_limits_fail_before_unbounded_processing() {
    let a = Identity::generate().unwrap();
    let b = Identity::generate().unwrap();
    assert!(Session::phone(&a, &[0; 32], PAIR, b.public_key()).is_err());
    assert!(Session::phone(&a, &PSK, [0; 16], b.public_key()).is_err());
    assert!(Session::phone(&a, &PSK, PAIR, [0; 32]).is_err());
    let (mut phone, _) = ready(&a, &b);
    assert!(phone.seal_record(&vec![0; 1024 * 1024 + 1]).is_err());
    assert!(phone.is_closed());
    let (_, mut desktop) = ready(&a, &b);
    assert!(desktop.open_frame(&vec![0; 65536]).is_err());
    assert!(desktop.is_closed());
}
