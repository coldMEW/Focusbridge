//! Cross-network relay client.
//!
//! Both devices dial the same public Workers endpoint on port 443, so neither
//! needs an inbound port, a public address, or a shared network. The relay routes
//! opaque binary frames between the two current sockets of one pair and can read
//! none of them: every application record is sealed inside a device-only Noise
//! session whose keys never leave this PC and the paired phone.
//!
//! Once that session is established the decrypted records are exactly the LAN
//! protocol envelopes, so they are handed to the existing local server over a
//! pinned loopback connection. Notification storage, acknowledgements, inventory
//! reconciliation, rules and diagnostics therefore run on one tested code path
//! regardless of which transport carried the bytes.

use crate::state::AppState;
use crate::sync::{relay_api, relay_identity};
use anyhow::{bail, Context, Result};
use focusbridge_core::relay::{control_type, fingerprint_bytes, pair_id_bytes};
use focusbridge_secure_channel::{Phase, Session};
use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::time::{sleep, timeout, Duration, Instant};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{
    connect_async_tls_with_config, Connector, MaybeTlsStream, WebSocketStream,
};
use tracing::{info, warn};

/// How long a fully idle relay socket may stay open before it is recycled. The
/// relay hibernates the connection, so this only bounds a socket that has stopped
/// carrying traffic without reporting an error.
const IDLE_TIMEOUT: Duration = Duration::from_secs(150);
const KEEPALIVE: Duration = Duration::from_secs(45);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const MIN_BACKOFF: Duration = Duration::from_secs(5);
const MAX_BACKOFF: Duration = Duration::from_secs(120);

type RelaySocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Explains once why the relay is not connected.
///
/// This used to be silent, which made "it will not connect" impossible to tell
/// apart from "it was never configured" without reading the source.
fn idle_once(state: &AppState, reason: &str) {
    if state.note_relay_idle(reason) {
        info!(reason, "relay idle");
    }
}

/// Supervises the relay connection for the lifetime of the app.
///
/// It is deliberately quiet when no relay pair is configured: cross-network sync
/// is opt-in, and LAN pairing must keep working with no account at all.
pub async fn start(state: AppState, local_port: u16) {
    let mut backoff = MIN_BACKOFF;
    loop {
        // Disconnect means disconnect. Paused outranks everything below it,
        // including the preference and a code being on screen, so nothing here
        // reaches for a phone the user just let go of.
        if state.is_paused() {
            idle_once(
                &state,
                "the phone was disconnected here; pick it under previous connections \
                 or ask for a new pairing code to reconnect",
            );
            state.await_relay_request().await;
            continue;
        }
        // Being at the relay is not the same as accepting a phone. This PC waits
        // there whenever a pairing code is on screen, because a phone that scans
        // it has no other way to reach this machine; whether the phone is then
        // let in is decided when it authenticates, by which code it presents.
        let automatic =
            relay_api::auto_connect(&state.db_path).unwrap_or(true) || state.pairing_code_is_live();
        if !automatic && !state.relay_connection_requested() {
            idle_once(
                &state,
                "waiting: automatic reconnection is off, so this PC joins the relay \
                 only when you pick a saved phone or press refresh on the pairing screen",
            );
            state.await_relay_request().await;
            // Re-evaluate rather than falling through: the wait also returns on
            // its safety-net timeout, and connecting then would be exactly the
            // automatic reconnection the user turned off.
            continue;
        }
        match attempt(&state, local_port).await {
            Ok(true) => backoff = MIN_BACKOFF,
            Ok(false) => {}
            Err(error) => {
                // Never log capabilities, key material, or notification content.
                warn!(error = %error, "relay session ended");
            }
        }
        sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

/// Returns true when a session was actually established, so the supervisor can
/// reset its backoff.
async fn attempt(state: &AppState, local_port: u16) -> Result<bool> {
    let Some(pair) = relay_api::current_pair(&state.db_path)? else {
        idle_once(state, "cross-network sync is not set up on this PC");
        return Ok(false);
    };
    if relay_identity::pair_secrets(&state.db_path, &pair.pair_id)?.is_none() {
        idle_once(
            state,
            "this relay pair has no local key material; set it up again",
        );
        return Ok(false);
    }
    let identity = relay_identity::identity(&state.db_path)?;

    let mut request = relay_api::socket_url(&pair)
        .into_client_request()
        .context("build relay socket request")?;
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", pair.desktop_capability)
            .parse()
            .context("build relay authorization header")?,
    );
    let (mut socket, _) = timeout(
        Duration::from_secs(20),
        connect_async_tls_with_config(request, None, false, None),
    )
    .await
    .context("relay connection timed out")?
    .context("connect to the relay")?;
    info!("relay socket established");
    // The request has been honoured; a later disconnect should not silently
    // reconnect when the user has asked for that not to happen.
    state.take_relay_request();

    // The relay reports the peer's presence on this same socket, so the desktop
    // can stay attached and wait rather than polling for the phone.
    let mut established = false;
    loop {
        let frame = match timeout(IDLE_TIMEOUT, socket.next()).await {
            Ok(Some(frame)) => frame.context("read relay frame")?,
            Ok(None) => break,
            Err(_) => bail!("relay socket idle timeout"),
        };
        match frame {
            Message::Text(text) => match control_type(&text).as_deref() {
                Some("relay.peer_ready") => {
                    established = true;
                    // Reload per-session: enrollment during the previous session may
                    // have pinned a phone that this one must now require.
                    let Some(secrets) =
                        relay_identity::pair_secrets(&state.db_path, &pair.pair_id)?
                    else {
                        break;
                    };
                    // One phone session at a time; a new generation always restarts.
                    if let Err(error) =
                        run_session(state, &mut socket, &identity, &secrets, &pair, local_port)
                            .await
                    {
                        warn!(error = %error, "relay phone session ended");
                        // Reconnect rather than waiting here. The relay announces a
                        // peer only when one joins, so a socket that outlives its
                        // session would sit waiting for an event that cannot arrive
                        // until the phone happens to reconnect first. Rejoining
                        // makes the relay announce this desktop to a phone that is
                        // still present, so recovery does not depend on the phone
                        // noticing that the session died.
                        break;
                    }
                }
                Some("relay.peer_unavailable") => continue,
                _ => continue,
            },
            // Outside a session there is no key that could open a binary frame;
            // stragglers from a retired session are simply dropped.
            Message::Binary(_) => continue,
            Message::Close(_) => break,
            _ => continue,
        }
    }
    Ok(established)
}

/// Runs one phone session: Noise handshake, mutual confirmation, then a pinned
/// loopback bridge into the existing local server.
async fn run_session(
    state: &AppState,
    socket: &mut RelaySocket,
    identity: &focusbridge_secure_channel::Identity,
    secrets: &relay_identity::PairSecrets,
    pair: &relay_api::RelayPair,
    local_port: u16,
) -> Result<()> {
    let pair_bytes = pair_id_bytes(&pair.pair_id)?;
    // A phone that is already pinned proves that identity on every connection.
    // Enrollment is the exception, and it is armed only when the user asks for a
    // phone by generating a fresh pairing code: reusing that state on every
    // reconnect would re-enroll a known phone instead of authenticating it, and
    // simply having the pairing screen open is not a request to replace anything.
    //
    // Arming it does authorize replacing an already-pinned phone, because a
    // handset that was reset, replaced or reinstalled has a new identity key and
    // could otherwise never pair with this PC again.
    // A pinned phone authenticates on every connection. Enrollment happens only
    // when the user has asked for a phone by generating a fresh pairing code.
    let mut session = match (secrets.phone, state.enrollment_armed()) {
        (Some(phone), false) => Session::desktop(identity, &secrets.psk, pair_bytes, phone),
        (_, true) => Session::desktop_enrollment(identity, &secrets.psk, pair_bytes),
        (None, false) => {
            bail!("no phone is enrolled for this relay pair; open the desktop pairing screen")
        }
    }
    .context("start the secure session")?;

    let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
    // The desktop responds: read message 1, write 2, read 3. The message count is
    // fixed by the profile, but the phase is authoritative here.
    while session.phase() == Phase::Handshake {
        let frame = next_binary(socket, deadline).await?;
        session
            .read_handshake(&frame)
            .map_err(|error| anyhow::anyhow!("relay handshake rejected: {error}"))?;
        if session.phase() == Phase::Handshake {
            let reply = session
                .write_handshake()
                .map_err(|error| anyhow::anyhow!("relay handshake failed: {error}"))?;
            send_binary(socket, reply, deadline).await?;
        }
    }

    if session.phase() == Phase::AwaitingApproval {
        let peer = session.peer_identity().context("missing phone identity")?;
        let binding = session.binding_hash().context("missing session binding")?;
        // Persist the pin before confirming, so a crash cannot leave a phone that
        // believes it is enrolled while this PC would re-open enrollment.
        relay_identity::approve_phone(&state.db_path, &pair.pair_id, peer)?;
        // One code, one enrollment. Later reconnects must prove this identity.
        state.disarm_enrollment();
        session
            .approve_enrollment(peer, binding)
            .map_err(|error| anyhow::anyhow!("enrollment approval failed: {error}"))?;
        info!("relay phone enrolled");
    }

    let confirmation = session
        .write_confirmation()
        .map_err(|error| anyhow::anyhow!("confirmation failed: {error}"))?;
    send_binary(socket, confirmation, deadline).await?;
    let reply = next_binary(socket, deadline).await?;
    session
        .read_confirmation(&reply)
        .map_err(|error| anyhow::anyhow!("phone confirmation rejected: {error}"))?;
    if !session.is_ready() {
        bail!("secure session did not reach the ready state");
    }
    info!("relay secure session ready");

    bridge(state, socket, &mut session, local_port).await
}

async fn next_binary(socket: &mut RelaySocket, deadline: Instant) -> Result<Vec<u8>> {
    loop {
        let frame = timeout(
            deadline.saturating_duration_since(Instant::now()),
            socket.next(),
        )
        .await
        .context("relay handshake timed out")?
        .context("relay socket closed during the handshake")?
        .context("read relay frame")?;
        match frame {
            Message::Binary(bytes) => return Ok(bytes),
            Message::Text(text)
                if control_type(&text).as_deref() == Some("relay.peer_unavailable") =>
            {
                bail!("the phone left before the session was established")
            }
            Message::Close(_) => bail!("relay socket closed during the handshake"),
            _ => continue,
        }
    }
}

async fn send_binary(socket: &mut RelaySocket, frame: Vec<u8>, deadline: Instant) -> Result<()> {
    timeout(
        deadline.saturating_duration_since(Instant::now()),
        socket.send(Message::Binary(frame)),
    )
    .await
    .context("relay write timed out")?
    .context("send relay frame")
}

/// Pumps decrypted records between the secure relay session and the local server.
async fn bridge(
    state: &AppState,
    socket: &mut RelaySocket,
    session: &mut Session,
    local_port: u16,
) -> Result<()> {
    let mut local = connect_local(state, local_port).await?;
    let mut keepalive = tokio::time::interval(KEEPALIVE);
    keepalive.tick().await;
    let mut idle = Instant::now();

    let result: Result<()> = async {
        loop {
            if idle.elapsed() >= IDLE_TIMEOUT {
                bail!("relay session idle timeout");
            }
            // Enforce the native session budgets even while no traffic arrives.
            session
                .check_alive()
                .map_err(|error| anyhow::anyhow!("secure session expired: {error}"))?;
            tokio::select! {
                _ = keepalive.tick() => {
                    socket.send(Message::Ping(Vec::new())).await.context("relay keepalive")?;
                }
                inbound = socket.next() => {
                    let Some(frame) = inbound else { bail!("relay socket closed") };
                    match frame.context("read relay frame")? {
                        Message::Binary(bytes) => {
                            idle = Instant::now();
                            if let Some(plaintext) = session
                                .open_frame(&bytes)
                                .map_err(|error| anyhow::anyhow!("relay frame rejected: {error}"))?
                            {
                                let text = String::from_utf8(plaintext.to_vec())
                                    .context("phone sent a non-text record")?;
                                local.send(Message::Text(text)).await.context("forward to the local server")?;
                            }
                        }
                        Message::Text(text) => {
                            if control_type(&text).as_deref() == Some("relay.peer_unavailable") {
                                bail!("the phone disconnected");
                            }
                        }
                        Message::Close(_) => bail!("relay socket closed"),
                        Message::Pong(_) | Message::Ping(_) | Message::Frame(_) => {}
                    }
                }
                outbound = local.next() => {
                    let Some(frame) = outbound else { bail!("local server closed the session") };
                    match frame.context("read from the local server")? {
                        Message::Text(text) => {
                            for sealed in session
                                .seal_record(text.as_bytes())
                                .map_err(|error| anyhow::anyhow!("seal failed: {error}"))?
                            {
                                socket.send(Message::Binary(sealed)).await.context("send relay frame")?;
                            }
                        }
                        Message::Ping(payload) => {
                            // The local server probes the transport every few
                            // seconds and closes the session if it goes
                            // unanswered. Replying here rather than relying on
                            // the queued automatic pong, which is only flushed
                            // when this side happens to write, and a quiet phone
                            // gives it nothing to write for far longer than the
                            // probe allows.
                            local
                                .send(Message::Pong(payload))
                                .await
                                .context("answer the local transport probe")?;
                        }
                        Message::Close(_) => bail!("local server closed the session"),
                        // The local server's transport probes stop at this bridge.
                        // End-to-end liveness is the phone's own PING/PONG envelopes,
                        // and this session dies with the relay socket either way.
                        _ => {}
                    }
                }
            }
        }
    }
    .await;

    session.close();
    // Closing the loopback socket makes the local server mark the phone
    // disconnected immediately instead of waiting for a heartbeat to lapse.
    let _ = timeout(Duration::from_secs(2), local.close(None)).await;
    result
}

/// Opens the loopback connection to this app's own WebSocket server, pinned to the
/// exact certificate the server presents. Nothing else on the machine can answer.
async fn connect_local(
    state: &AppState,
    port: u16,
) -> Result<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>> {
    let expected = fingerprint_bytes(&state.cert.fingerprint_sha256_hex)?;
    let verifier = Arc::new(PinnedCertificate { expected });
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    // Address the loopback interface directly: "localhost" can resolve to ::1
    // first, and the listener binds IPv4. The pinning verifier ignores the name,
    // so there is no certificate-name dependency here.
    let request = format!("wss://127.0.0.1:{port}/")
        .into_client_request()
        .context("build loopback request")?;
    let (socket, _) = timeout(
        Duration::from_secs(10),
        connect_async_tls_with_config(
            request,
            None,
            false,
            Some(Connector::Rustls(Arc::new(config))),
        ),
    )
    .await
    .context("loopback connection timed out")?
    .context("connect to the local FocusBridge server")?;
    Ok(socket)
}

/// Accepts exactly one certificate, by SHA-256 of its DER encoding — the same pin
/// the phone applies over the LAN. Chain building and name matching are irrelevant
/// for a self-signed certificate this process generated itself.
#[derive(Debug)]
struct PinnedCertificate {
    expected: [u8; 32],
}

impl rustls::client::danger::ServerCertVerifier for PinnedCertificate {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        // A pinned self-signed certificate is presented alone; extra certificates
        // would mean this is not the server we generated.
        if !intermediates.is_empty() {
            return Err(rustls::Error::General(
                "unexpected certificate chain from the local server".into(),
            ));
        }
        let actual: [u8; 32] = Sha256::digest(end_entity.as_ref()).into();
        if actual == self.expected {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(
                "local certificate does not match the pinned fingerprint".into(),
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
