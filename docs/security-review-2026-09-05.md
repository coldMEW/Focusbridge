# Relay and mobile-lock review

Baseline: ebe1356. This checkpoint is a scoped source review, not certification
that every line or feature in the repository is correct.

## Reviewed and changed

- Relay routing/session code: failed channel sends now retain pending phone
  messages, failed queue flushes preserve messages and their original expiry,
  and stale socket cleanup cannot detach a replacement. Routing verifies sender
  ownership while holding the session lock. A successful enqueue is not a
  destination delivery acknowledgement.
- Relay WebSocket task: writer failure stops the idle reader; writes and close
  attempts have deadlines; periodic ownership checks retire replaced sessions.
  Configured protocol pings now run, with an idle timeout of three intervals.
- Relay message budgets: per-pair, per-role limits are enforced and survive
  reconnects. This is NOT HTTP authentication/OTP rate limiting.
- Relay startup config: no silent TLS downgrade and no ephemeral fallback signing
  secret. Missing/short signing secrets and zero limits reject startup. Explicit
  `allow_plaintext = true` is only for a trusted TLS proxy or local development;
  a partially missing TLS certificate/key always rejects startup.
- Relay authentication helpers: exact token expiry boundary and malformed or
  excessive-work password hashes fail validation.
- Relay account persistence: write/sync a same-directory temporary file and
  replace the old file before publishing new in-memory accounts. Failed saves
  do not create ghost accounts. Password verification runs outside the store lock.
- Android app-lock code and UI wiring: atomic credential loading/writes,
  fail-closed missing hashes, background relock, stale-unlock prevention,
  gated deep-link/reconnect actions, and persistent serialized attempt throttling.
  PIN/password and recovery-answer fields are masked and disable autocorrect.

## Verification

- Relay MSVC `cargo test --offline`: 37 tests passed. The older integration
  placeholder files still contain zero tests; they are not counted as coverage.
- Relay Clippy all targets with warnings denied, rustfmt, and whitespace checks
  passed. Socket lifecycle was source-reviewed; no live relay integration test
  is claimed.
- Android Gradle 8.7 `testDebugUnitTest assembleDebug --offline`: 46 tests passed
  and the debug APK built. Security Compose source guardrails are not UI/device
  tests.
- Android `lint`: passed after allowing missing public dependencies to download
  into the existing Gradle cache. No cache deletion or lint baseline suppression.

## Compatibility notes

Relay operators now need a persistent `FOCUSBRIDGE_AUTH_TOKEN_SECRET` (at least
32 bytes; use a cryptographically random value) and valid TLS files, or an
explicit trusted-proxy plaintext opt-in. Do not expose the relay directly with
plaintext enabled. Existing LAN clients do not use these relay startup settings.

The mobile app lock remains optional. When enabled, leaving the activity locks
its UI; existing background notification sync is not disabled by locking the UI.

## Remaining work and review coverage

- Public relay still lacks device-only encryption keys separated from server
  credentials, message replay protection, durable pair ownership/revocation,
  and verified desktop relay integration. Keep cloud mode blocked.
- HTTP login/registration/OTP routes need abuse controls, bounded pending OTP
  storage, off-thread password hashing, and account/email-linking review. The
  legacy direct registration route does not establish verified email ownership.
- A WebSocket channel enqueue is not a durable end-device ACK. Process crashes
  can still lose relay memory; bounded local retry/deduplication must be verified
  against the final encrypted relay protocol.
- Android throttle uses wall-clock persistence; clock manipulation needs further
  hardening. Local recovery questions are not strong account recovery factors.
- No exhaustive re-audit of desktop auth/storage/UI, Android notification parsers,
  release signing, Firebase rules, or all dependency source code in this slice.
- No physical-device overnight/Doze test, actual relay deployment, or production
  installer release is implied by passing unit tests.

Free-first remains mandatory. No paid services, subscriptions, or trials were
started. Free relay setup guidance remains in `free-relay-options.md`.

## Encrypted-storage staging and session design

This is unfinished source, not an active security feature. The user's live
databases have not been migrated. Do not ship or count gated tests as passing.

Desktop staging files: `src-tauri/src/db/encrypted.rs`,
`src-tauri/src/security/database_key.rs`, and
`src-tauri/tests/encrypted_database.rs` under `desktop/`. The integration test
currently uses `#![cfg(any())]` until its native dependency and module wiring are
coordinated. Existing database connections are unchanged.

Resume desktop storage in one build slot:

1. Resolve MSVC-native Perl/OpenSSL tooling; use the documented rusqlite
   SQLCipher vendored-OpenSSL feature instead of ordinary `bundled`. Add `fs2`,
   `zeroize`, `tempfile`, and the required Windows DPAPI/file API features.
   Regenerate/review `desktop/Cargo.lock` through Cargo.
2. Expose the new modules, route all store opens through the encrypted factory,
   and retain its lifetime guard via Tauri managed state before starting sync.
   Adapt inventory fixtures to explicit test keys and guards; no live DB use.
3. Remove the staging gate, execute all migration/crash/WAL/key-loss fixtures,
   independently test unreadability with ordinary SQLite, then regressions and
   Clippy. Review partial-export recovery, implicit rowid preservation, and
   non-Windows behavior before activation. Non-Windows key storage is currently
   unsupported; do not silently fall back to plaintext.

Staged recovery keeps plaintext migration copies until encrypted validation
succeeds. Partial/corrupt or mismatched copies fail closed for manual recovery.
Deleting legacy copies cannot guarantee forensic erasure on SSDs/backups.

For message sessions, review favored one shared Rust Noise engine on desktop
and Android through JNI instead of porting cryptography. Snow 0.10.0 is a
candidate, not an integrated or audited app protocol. The Java candidates
reviewed did not establish a maintained drop-in Android API-26 implementation
of the proposed modern handshake. First prove Android native packaging,
published vectors, peer authentication, replay rejection, reconnect, and rekey
boundaries. Then wire identity/enrollment and separate relay capabilities.
Current envelope crypto has not been replaced; cloud mode must stay blocked.

Android staging includes Keystore-wrapped database keys, encrypted migration
helpers, JVM state/key fixtures, and 14 compiled native fixtures. Its existing
Room provider still opens plaintext; `buildEncryptedDatabase` is not called.
61 JVM tests passed (15 added storage tests), but native tests have not run.
Use a disposable emulator and matching fresh target/test APKs before wiring
the provider. Add genuine process-death and storage-failure checks beyond
exception injection. Native packaging, lint warnings, backup/key-loss policy,
and all existing schema migrations need disposition before shipping.

Sources: [rusqlite](https://github.com/rusqlite/rusqlite/tree/v0.31.0),
[Snow release](https://github.com/mcginty/snow/releases/tag/v0.10.0),
[Noise specification](https://noiseprotocol.org/noise.html),
[Java Noise candidate](https://github.com/jchambers/java-noise).
