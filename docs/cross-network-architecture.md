# Cross-network sync architecture

How a phone on mobile data reaches a PC on home Wi-Fi, and what each component is
trusted with. Written 2026-09-05, after the relay was deployed and the shared
Noise engine was integrated into both clients.

## The problem LAN pairing cannot solve

The original transport was a QR-pinned WSS listener on the desktop at
`0.0.0.0:9173`. That works whenever the phone can open a TCP connection to the
PC's address, which means the same Wi-Fi or the same hotspot. It cannot work when
the phone is on mobile data, on a guest network with client isolation, or behind
any NAT that does not forward a port — there is simply no route to dial.

The fix is not to make the PC reachable. It is to have **both** devices dial out
to a shared rendezvous point on port 443, which every network allows.

## Transports

FocusBridge now has two transports carrying one identical application protocol.

| | LAN | Relay |
|---|---|---|
| Reachability | Same network or hotspot | Any network with outbound HTTPS |
| Needs an account | No | Yes, a verified one |
| Needs the Internet | No | Yes |
| Transport security | TLS 1.3, self-signed cert pinned from the QR | TLS to Cloudflare, plus an end-to-end Noise session |
| Who can read content | The two devices | The two devices |
| Frame type | WebSocket text | WebSocket binary |

The phone always tries every LAN candidate first and only falls back to the relay
when none answers (`SyncEngine.connectActivePairing`). The LAN path is faster, has
no third party in it, and keeps working with no account and no Internet — so it
stays the default rather than becoming legacy.

## The relay

`relay-worker/` is a Cloudflare Worker with one SQLite-backed Durable Object per
hashed account identity, deployed at
`https://focusbridge-relay.focusbridge.workers.dev`.

The owner authenticates with a **Firebase ID token** (verified signature, issuer,
audience, expiry, `auth_time`, and `email_verified`) to `POST /v1/pairs`. The
relay mints two random capabilities, one per role, stores only their hashes, and
returns both once. The desktop keeps its own; the phone's travels in the QR.

Both devices then open
`wss://<relay>/v1/socket/{accountKey}/{pairId}/{role}` with
`Authorization: Bearer <capability>`. The Durable Object holds at most one live
socket per pair and role, and forwards **opaque binary frames** to the current
opposite role. It enforces a 64 KiB frame cap, a persisted per-role token bucket,
session byte and message budgets, a 10-pair-per-account limit, and a 20-pair
creations-per-hour budget that survives pair deletion. It uses the WebSocket
Hibernation API, so an idle pair costs no wall-clock time.

What the relay explicitly cannot do:

- Read any application content. It never holds a message key.
- Manufacture an application acknowledgement, or report a phone as authenticated.
- Retain application data. There is no queue: if the peer is absent it says
  `relay.peer_unavailable` and durable retry stays on the endpoints.

Capabilities are routing tokens. Revoking a pair (`DELETE /v1/pairs/{pairId}`)
stops routing immediately and is independent of the device key material below.

## The device-only session

Relay authorization is not message security. Every application record crossing
the relay is sealed in a `Noise_XXpsk3_25519_ChaChaPoly_SHA256` session
implemented once, in Rust, in `shared/secure-channel/`. Windows links the crate
directly; Android loads the same code through
`shared/secure-channel-jni/` as `libfocusbridge_secure_channel_jni.so`, built for
`arm64-v8a`, `armeabi-v7a`, `x86_64` and `x86`. There is no second implementation
to disagree with the first, and no plaintext fallback.

- The phone initiates, the desktop responds. Roles cannot be swapped by a frame.
- The prologue binds the protocol version and the random pair ID, so a session
  cannot be replayed into a different pairing.
- The QR carries a random 32-byte enrollment PSK and the desktop's static public
  key. Neither is a relay capability. The phone validates the desktop identity
  before sending its final handshake message.
- After the handshake both sides exchange a bound confirmation record. Until that
  completes, no application frame is accepted in either direction.
- The desktop pins the phone's static key at enrollment and requires it on every
  later session. Enrollment is only possible while the desktop pairing screen has
  a live QR, and a pair that already has a phone refuses a different one.
- Ordered counters reject replay and reflection. Sessions are bounded by message
  count, bytes and age. Any authentication or decoding failure closes the session
  permanently; the transport reconnects rather than retrying a frame.
- Records larger than one frame are chunked with authenticated headers, so a
  truncated or reordered app inventory never releases a prefix.

## Why the desktop bridges over loopback

Once the Noise session is ready, the decrypted records are byte-for-byte the same
JSON envelopes the LAN transport carries. Rather than duplicating the application
layer, `sync/relay_client.rs` connects to the desktop's own listener on
`localhost:9173` — pinned to the exact certificate this process generated, by
SHA-256 of its DER — and pumps records through it.

Notification storage, delivery acknowledgements, inventory reconciliation, rules
sync, connection diagnostics and duplicate suppression therefore run on exactly
one tested code path no matter which transport delivered the bytes. Closing the
relay session closes the loopback socket, so the local server marks the phone
disconnected at once instead of waiting for a heartbeat to lapse.

This adds a local TLS hop. It does not widen exposure: `:9173` already accepts
LAN connections and already requires the pairing key to authenticate.

## Connection state

A relay socket is a route, not a peer. The phone reports `CONNECTED` only after
the Noise session is ready **and** the desktop has answered `AUTH_OK`. While the
relay reports `relay.peer_unavailable` the phone stays attached — so the desktop's
arrival is delivered on the same socket without polling — but the UI shows
disconnected, and `isAwaitingPeer()` keeps the supervisor from tearing the socket
down every retry tick.

## Key storage

| Secret | Where | Wrapped by |
|---|---|---|
| Desktop Noise static key | `settings` table | SQLCipher, key in Windows DPAPI |
| Desktop relay capability | `settings` table | as above |
| Per-pair PSK, pinned phone key | `settings` table | as above |
| Phone Noise static key | `no_backup/focusbridge-device-identity.v1` | AES-GCM key in Android Keystore |
| Phone relay capability, PSK, pinned desktop key | Room `pairings` row | Room database |

The phone identity and the database passphrase use distinct keystore aliases,
records and additional authenticated data, so neither wrapped secret can be
substituted for the other. Neither key is rotated automatically: the peer pins the
matching public half, so silently replacing one would present the device as
unknown rather than recover the pairing.

## What this does not promise

- It cannot reach a phone that is powered off, force-stopped, out of coverage, or
  behind an administrator who blocks the service.
- `workers.dev` is documented by Cloudflare as suitable for personal use. A
  business-critical deployment wants a custom domain and a route.
- Free-tier limits are not permanent guarantees and fail operations rather than
  silently granting capacity. Recheck them before relying on them.
- Notification mirroring is not SMS, call, media, clipboard or file access. See
  `phone-link-gap-analysis.md` for the honest feature comparison.
