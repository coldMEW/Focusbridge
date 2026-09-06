//! Device-only authenticated sessions. Never use relay capabilities as enrollment PSKs.
//! A session is single-owner, ordered, and permanently discarded on every protocol error.

#[cfg(test)]
mod limits_tests;
mod records;

use snow::resolvers::{CryptoResolver, DefaultResolver};
use snow::{Builder, HandshakeState, TransportState};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

pub use records::{MAX_FRAME, MAX_RECORD};
pub const PROFILE: &str = "Noise_XXpsk3_25519_ChaChaPoly_SHA256";
const HANDSHAKE_MAX: usize = 256;
const MAX_MESSAGES: u64 = 1_000_000;
const MAX_BYTES: u64 = 1 << 30;
const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const HANDSHAKE_AGE: Duration = Duration::from_secs(120);
const DESKTOP_CONFIRM: &[u8] = b"FocusBridge/v2/desktop-ready";
const PHONE_CONFIRM: &[u8] = b"FocusBridge/v2/phone-ready";

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    InvalidState,
    InvalidInput,
    AuthenticationFailed,
    InitializationFailed,
    LimitReached,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidState => "secure session is not in the required state",
            Self::InvalidInput => "invalid secure-session frame or configuration",
            Self::AuthenticationFailed => "secure-session authentication failed",
            Self::InitializationFailed => "secure session could not be initialized",
            Self::LimitReached => "secure-session limit reached; reconnect required",
        })
    }
}
impl std::error::Error for Error {}

// No Debug/Clone implementation: identity secrets must not enter logs by accident.
pub struct Identity {
    private: Zeroizing<[u8; 32]>,
    public: [u8; 32],
}
impl Identity {
    pub fn generate() -> Result<Self, Error> {
        let keys = Builder::new(PROFILE.parse().map_err(|_| Error::InitializationFailed)?)
            .generate_keypair()
            .map_err(|_| Error::InitializationFailed)?;
        let private = Zeroizing::new(keys.private);
        Self::from_private(
            private
                .as_slice()
                .try_into()
                .map_err(|_| Error::InitializationFailed)?,
        )
    }
    pub fn from_private(private: [u8; 32]) -> Result<Self, Error> {
        let private = Zeroizing::new(private);
        if private.iter().all(|byte| *byte == 0) {
            return Err(Error::InvalidInput);
        }
        let mut dh = DefaultResolver
            .resolve_dh(&snow::params::DHChoice::Curve25519)
            .ok_or(Error::InitializationFailed)?;
        dh.set(private.as_slice());
        let public = dh
            .pubkey()
            .try_into()
            .map_err(|_| Error::InitializationFailed)?;
        Ok(Self { private, public })
    }
    /// Persist only inside platform-protected local storage. Never send to a relay.
    pub fn export_private(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(*self.private)
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.public
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Handshake,
    AwaitingApproval,
    SendDesktopConfirmation,
    AwaitingDesktopConfirmation,
    SendPhoneConfirmation,
    AwaitingPhoneConfirmation,
    Ready,
    Closed,
}
pub struct Session {
    handshake: Option<HandshakeState>,
    transport: Option<TransportState>,
    phase: Phase,
    phone: bool,
    expected_peer: Option<[u8; 32]>,
    peer: Option<[u8; 32]>,
    binding: Option<[u8; 32]>,
    created: Instant,
    sent_messages: u64,
    sent_bytes: u64,
    received_messages: u64,
    received_bytes: u64,
    outgoing_record: u64,
    incoming: records::Assembler,
}
impl Session {
    pub fn phone(
        identity: &Identity,
        psk: &[u8; 32],
        pair: [u8; 16],
        desktop: [u8; 32],
    ) -> Result<Self, Error> {
        Self::new(identity, psk, pair, true, Some(desktop))
    }
    pub fn desktop(
        identity: &Identity,
        psk: &[u8; 32],
        pair: [u8; 16],
        phone: [u8; 32],
    ) -> Result<Self, Error> {
        Self::new(identity, psk, pair, false, Some(phone))
    }
    /// Only a locally opened enrollment invitation may call this constructor.
    /// Never use it as fallback for a missing, corrupt, or mismatched saved pin.
    pub fn desktop_enrollment(
        identity: &Identity,
        psk: &[u8; 32],
        pair: [u8; 16],
    ) -> Result<Self, Error> {
        Self::new(identity, psk, pair, false, None)
    }
    fn new(
        identity: &Identity,
        psk: &[u8; 32],
        pair: [u8; 16],
        phone: bool,
        peer: Option<[u8; 32]>,
    ) -> Result<Self, Error> {
        if psk.iter().all(|byte| *byte == 0) || pair == [0; 16] || peer == Some([0; 32]) {
            return Err(Error::InvalidInput);
        }
        let mut prologue = b"FocusBridge/session/v2/notification-sync/phone-to-desktop/".to_vec();
        prologue.extend_from_slice(&pair);
        let builder = Builder::new(PROFILE.parse().map_err(|_| Error::InitializationFailed)?)
            .local_private_key(identity.private.as_slice())
            .map_err(|_| Error::InitializationFailed)?
            .psk(3, psk)
            .map_err(|_| Error::InitializationFailed)?
            .prologue(&prologue)
            .map_err(|_| Error::InitializationFailed)?;
        let handshake = if phone {
            builder.build_initiator()
        } else {
            builder.build_responder()
        }
        .map_err(|_| Error::InitializationFailed)?;
        Ok(Self {
            handshake: Some(handshake),
            transport: None,
            phase: Phase::Handshake,
            phone,
            expected_peer: peer,
            peer: None,
            binding: None,
            created: Instant::now(),
            sent_messages: 0,
            sent_bytes: 0,
            received_messages: 0,
            received_bytes: 0,
            outgoing_record: 0,
            incoming: records::Assembler::default(),
        })
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn is_ready(&self) -> bool {
        self.phase == Phase::Ready
    }
    pub fn is_closed(&self) -> bool {
        self.phase == Phase::Closed
    }
    pub fn peer_identity(&self) -> Option<[u8; 32]> {
        self.peer
    }
    pub fn binding_hash(&self) -> Option<[u8; 32]> {
        self.binding
    }
    pub fn close(&mut self) {
        self.phase = Phase::Closed;
        self.handshake = None;
        self.transport = None;
        self.incoming = records::Assembler::default();
        self.peer = None;
        self.binding = None;
    }
    fn checked<T>(&mut self, op: impl FnOnce(&mut Self) -> Result<T, Error>) -> Result<T, Error> {
        self.checked_at(Instant::now(), op)
    }
    fn checked_at<T>(
        &mut self,
        now: Instant,
        op: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let result = if self.is_closed() {
            Err(Error::InvalidState)
        } else if self.incoming.expired()
            || now.saturating_duration_since(self.created)
                >= if self.is_ready() {
                    MAX_AGE
                } else {
                    HANDSHAKE_AGE
                }
        {
            Err(Error::LimitReached)
        } else {
            op(self)
        };
        if result.is_err() {
            self.close();
        }
        result
    }
    /// Invoke from the transport supervisor even while no application data arrives.
    pub fn check_alive(&mut self) -> Result<(), Error> {
        self.checked(|_| Ok(()))
    }
    pub fn write_handshake(&mut self) -> Result<Vec<u8>, Error> {
        self.checked(|session| {
            if session.phase != Phase::Handshake {
                return Err(Error::InvalidState);
            }
            let hs = session.handshake.as_mut().ok_or(Error::InvalidState)?;
            if !hs.is_my_turn() {
                return Err(Error::InvalidState);
            }
            let mut frame = vec![0; HANDSHAKE_MAX];
            let length = hs
                .write_message(&[], &mut frame)
                .map_err(|_| Error::AuthenticationFailed)?;
            frame.truncate(length);
            session.finish_handshake()?;
            Ok(frame)
        })
    }
    pub fn read_handshake(&mut self, frame: &[u8]) -> Result<(), Error> {
        self.checked(|session| {
            if session.phase != Phase::Handshake {
                return Err(Error::InvalidState);
            }
            if frame.is_empty() || frame.len() > HANDSHAKE_MAX {
                return Err(Error::InvalidInput);
            }
            let hs = session.handshake.as_mut().ok_or(Error::InvalidState)?;
            if hs.is_my_turn() {
                return Err(Error::InvalidState);
            }
            let mut payload = Zeroizing::new([0u8; HANDSHAKE_MAX]);
            let length = hs
                .read_message(frame, payload.as_mut())
                .map_err(|_| Error::AuthenticationFailed)?;
            if length != 0 {
                return Err(Error::InvalidInput);
            }
            // Pin the actual learned static key, not Builder::remote_public_key:
            // XX overwrites its remote static slot while processing the handshake.
            if let Some(remote) = hs.get_remote_static() {
                let remote: [u8; 32] =
                    remote.try_into().map_err(|_| Error::AuthenticationFailed)?;
                if session.expected_peer.is_some_and(|pin| pin != remote) {
                    return Err(Error::AuthenticationFailed);
                }
                session.peer = Some(remote);
            }
            session.finish_handshake()
        })
    }
    fn finish_handshake(&mut self) -> Result<(), Error> {
        if !self
            .handshake
            .as_ref()
            .ok_or(Error::InvalidState)?
            .is_handshake_finished()
        {
            return Ok(());
        }
        if self.peer.is_none() {
            return Err(Error::AuthenticationFailed);
        }
        let handshake = self.handshake.take().ok_or(Error::InvalidState)?;
        self.binding = Some(
            handshake
                .get_handshake_hash()
                .try_into()
                .map_err(|_| Error::AuthenticationFailed)?,
        );
        self.transport = Some(
            handshake
                .into_transport_mode()
                .map_err(|_| Error::AuthenticationFailed)?,
        );
        self.phase = if self.phone {
            Phase::AwaitingDesktopConfirmation
        } else if self.expected_peer.is_none() {
            Phase::AwaitingApproval
        } else {
            Phase::SendDesktopConfirmation
        };
        Ok(())
    }
    /// The caller must atomically save the approved pin and consume its enrollment
    /// invitation before calling this. Approval is bound to this exact handshake.
    pub fn approve_enrollment(&mut self, peer: [u8; 32], binding: [u8; 32]) -> Result<(), Error> {
        self.checked(|session| {
            if session.phase != Phase::AwaitingApproval
                || session.peer != Some(peer)
                || session.binding != Some(binding)
            {
                return Err(Error::InvalidState);
            }
            session.expected_peer = Some(peer);
            session.phase = Phase::SendDesktopConfirmation;
            Ok(())
        })
    }
    pub fn write_confirmation(&mut self) -> Result<Vec<u8>, Error> {
        self.checked(|session| {
            let (plaintext, phase) = match session.phase {
                Phase::SendDesktopConfirmation => {
                    (DESKTOP_CONFIRM, Phase::AwaitingPhoneConfirmation)
                }
                Phase::SendPhoneConfirmation => (PHONE_CONFIRM, Phase::Ready),
                _ => return Err(Error::InvalidState),
            };
            let mut bound = plaintext.to_vec();
            bound.extend_from_slice(&session.binding.ok_or(Error::InvalidState)?);
            let frame = session.encrypt(&bound)?;
            session.phase = phase;
            Ok(frame)
        })
    }
    pub fn read_confirmation(&mut self, frame: &[u8]) -> Result<(), Error> {
        self.checked(|session| {
            let (expected, phase) = match session.phase {
                Phase::AwaitingDesktopConfirmation => {
                    (DESKTOP_CONFIRM, Phase::SendPhoneConfirmation)
                }
                Phase::AwaitingPhoneConfirmation => (PHONE_CONFIRM, Phase::Ready),
                _ => return Err(Error::InvalidState),
            };
            let mut bound = expected.to_vec();
            bound.extend_from_slice(&session.binding.ok_or(Error::InvalidState)?);
            if session.decrypt(frame)?.as_slice() != bound {
                return Err(Error::AuthenticationFailed);
            }
            session.phase = phase;
            Ok(())
        })
    }
    fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, Error> {
        if plaintext.len() + 16 > MAX_FRAME {
            return Err(Error::InvalidInput);
        }
        if self.sent_messages >= MAX_MESSAGES
            || self.sent_bytes + plaintext.len() as u64 > MAX_BYTES
        {
            return Err(Error::LimitReached);
        }
        let mut frame = vec![0; plaintext.len() + 16];
        let len = self
            .transport
            .as_mut()
            .ok_or(Error::InvalidState)?
            .write_message(plaintext, &mut frame)
            .map_err(|_| Error::AuthenticationFailed)?;
        frame.truncate(len);
        self.sent_messages += 1;
        self.sent_bytes += plaintext.len() as u64;
        Ok(frame)
    }
    fn decrypt(&mut self, frame: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error> {
        if frame.len() < 16 || frame.len() > MAX_FRAME {
            return Err(Error::InvalidInput);
        }
        if self.received_messages >= MAX_MESSAGES
            || self.received_bytes + (frame.len() - 16) as u64 > MAX_BYTES
        {
            return Err(Error::LimitReached);
        }
        let mut plaintext = Zeroizing::new(vec![0; frame.len() - 16]);
        let len = self
            .transport
            .as_mut()
            .ok_or(Error::InvalidState)?
            .read_message(frame, &mut plaintext)
            .map_err(|_| Error::AuthenticationFailed)?;
        plaintext.truncate(len);
        self.received_messages += 1;
        self.received_bytes += len as u64;
        Ok(plaintext)
    }
    pub fn seal_record(&mut self, plaintext: &[u8]) -> Result<Vec<Vec<u8>>, Error> {
        self.checked(|session| {
            if !session.is_ready() {
                return Err(Error::InvalidState);
            }
            if plaintext.is_empty() || plaintext.len() > MAX_RECORD {
                return Err(Error::InvalidInput);
            }
            let mut frames = Vec::with_capacity(plaintext.len().div_ceil(records::CHUNK));
            for (index, chunk) in plaintext.chunks(records::CHUNK).enumerate() {
                let mut contents =
                    Zeroizing::new(Vec::with_capacity(records::HEADER + chunk.len()));
                contents.extend_from_slice(&session.outgoing_record.to_be_bytes());
                contents.extend_from_slice(&(plaintext.len() as u32).to_be_bytes());
                contents.extend_from_slice(&((index * records::CHUNK) as u32).to_be_bytes());
                contents.extend_from_slice(chunk);
                frames.push(session.encrypt(&contents)?);
            }
            session.outgoing_record = session
                .outgoing_record
                .checked_add(1)
                .ok_or(Error::LimitReached)?;
            Ok(frames)
        })
    }
    pub fn open_frame(&mut self, frame: &[u8]) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        self.checked(|session| {
            if !session.is_ready() {
                return Err(Error::InvalidState);
            }
            let plaintext = session.decrypt(frame)?;
            session.incoming.push(&plaintext)
        })
    }
}
