# Secure Relay Execution Plan

**Goal:** Complete the security prerequisites, then connect approved Windows and
Android devices across networks without revealing message keys to the relay.

**Architecture:** LAN stays available offline. Both devices can dial a public
Workers WSS endpoint on port 443. Relay capabilities authorize routing only;
device-only Noise sessions authenticate and encrypt application envelopes.
Local storage uses SQLCipher with platform-wrapped random keys. No insecure
fallback or live data reset is permitted.

**Tech Stack:** Existing React/Tauri/Kotlin/Room; SQLCipher, Windows DPAPI,
Android Keystore, Snow 0.10.0 shared Rust engine, Workers/SQLite Durable Objects.

## Wave 1 ownership and gates

- [ ] Windows storage owner: `desktop/src-tauri/src/db/**`,
  `src/security/database_key.rs`, `src/lib.rs`, Cargo manifest/lockfile,
  `tests/encrypted_database.rs`, and `tests/app_inventory.rs`. Activate existing
  staged fixtures, fix migrations, run all native temporary-database tests and
  Clippy. Do not launch the real app until review.
- [ ] Android storage owner: `android/app/src/main/java/.../storage/**`,
  `security/DatabaseKeyStore.kt`, `di/DatabaseModule.kt`, storage tests and Gradle
  dependencies only. Run 14 staged native tests on a disposable emulator, add
  process/storage-failure recovery tests, preserve schema migrations and data.
- [ ] Parent: `shared/secure-channel/**`. Build a narrow stateful Snow wrapper
  with identity/context validation, ordered transport and bounded framing; test
  before integrating into either existing client. No desktop Cargo manifest
  changes while the storage owner holds that build slot.
- [ ] Relay owner: `relay-worker/**` only. Implement/test authenticated pair
  provisioning and revocation plus opaque, bounded, hibernating WSS routing.
  No deployment, no private payloads, no app-client edits in this wave.

## Crypto contract for proof and review

- Profile `Noise_XXpsk3_25519_ChaChaPoly_SHA256` implemented by Snow, not ported
  cryptography. Phone initiates; desktop responds. Prologue binds protocol
  version and the random pair ID; roles cannot be switched by a frame.
- QR contains a random 32-byte enrollment PSK and desktop public identity key;
  neither is a relay auth capability. Phone validates desktop identity before
  sending final handshake bytes. Desktop validates the saved phone identity
  before application dispatch on subsequent sessions. Initial enrollment is
  gated by QR possession and explicit approval.
- Fresh ephemeral keys on every connection. No application frame accepted
  until handshake completes and identity matches. Authentication/decryption
  failure permanently closes that session. Reconnect starts a fresh session.
- Noise ordered counters reject replay/reflection. Bound total session message
  count and bytes; require a new handshake before exhaustion. Reject oversized
  inputs before allocating. Larger application records require authenticated
  bounded chunk assembly, not an unbounded single ciphertext.
- Durable queues retain application IDs/plaintext only inside encrypted local
  storage; re-encrypt on reconnect. The relay must not promise delivery of old
  session ciphertext after a handshake has changed.

## Relay contract for first implementation

- Firebase project `foucsbridge`; verify real Firebase ID-token signature,
  issuer, audience, subject, expiry, auth time, email verification for account
  provisioning. Cache public signing keys according to provider TTL. Never log
  bearer tokens, QR values, or notification content.
- One SQLite Durable Object per hashed account identity, maximum 10 pairs.
  Provision random per-role routing capabilities, persist hashes only, allow
  owner revocation. Capabilities expire; no anonymous pair creation.
- Routes: `GET /health`, authenticated owner `POST /v1/pairs`,
  `GET /v1/pairs`, `DELETE /v1/pairs/{pairId}`; native-client WSS at
  `/v1/socket/{accountKey}/{pairId}/{role}` with Authorization header.
  Account key and pair ID are routing metadata, never credentials.
- Exactly one current socket per pair/role. Reject stale replaced sockets,
  revoked/expired credentials, invalid roles, text application payloads,
  oversized frames, bursts, and more than bounded pair/socket counts.
- Route opaque binary frames only to the current opposite role. If peer is
  absent, report unavailable; durable retry belongs to the endpoint. Relay
  cannot manufacture application ACKs or report the phone authenticated.
- Use hibernation APIs and SQLite-backed DO migrations compatible with Free.
  Bounded control metadata and automatic keepalive responses only. No timers
  that keep a Durable Object permanently awake, no paid bindings.

## Subsequent integration gates

- [ ] Review independent components and resolve every finding before wiring.
- [ ] Shared native Android library packaging and Windows/Android actual
  handshake/vector tests. Versioned QR migration with no silent downgrade.
- [ ] Backend session integration on both clients, including peer-ready proof,
  durable idempotent rules/mode ACKs, inventory refresh, disconnect/revocation.
- [ ] Separate consented remote-control availability from data sync. Show
  waiting/offline/denied/expired states. Full disconnect cannot be remotely
  bypassed; paused notifications may retain an explicitly approved control path.
- [ ] Browser-authorize only deployment scopes required by the reviewed Worker.
  Confirm Free plan and dry-run before public deployment. Use synthetic clients
  first; real phone notifications only after crypto and storage acceptance.
- [ ] Real cellular-phone/PC-Wi-Fi, LAN isolation, route changes, sleep/Doze,
  reconnect, duplicates, quota/outage recovery and clean-machine release tests.
- [ ] Private release signing, installer and app UX acceptance; remaining
  Phone Link feature gaps stay labeled rather than advertised as implemented.

No commits or pushes until the complete release gate requested by the user.
