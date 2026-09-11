# FocusBridge blueprint

The single map of this repository: what every part is, where it lives, how the
pieces reach each other, and why each of the awkward parts is shaped the way it
is. Written so that someone — or something — arriving with no memory of the
previous sessions can find the right file on the first try.

Read alongside:

- [`behaviour-checklist.md`](behaviour-checklist.md) — every feature and fixed
  bug with the test that holds it. **Walk it before every commit.**
- [`PROJECT_MEMORY.md`](PROJECT_MEMORY.md) — the running session log.
- [`cross-network-architecture.md`](cross-network-architecture.md) — the relay
  design and threat notes in full.
- [`security-review-2026-09-06.md`](security-review-2026-09-06.md) — the current
  review, including what has *not* been done.
- [`apple/`](apple/) — research into extending the project to macOS and iPhone:
  what Apple permits, what it forbids, and a phased plan. Start at
  [`apple/00-verdict.md`](apple/00-verdict.md).

Last updated: 2026-09-07, at commit `cdf418f`.

---

## 1. What the product is

An Android phone captures its notifications, filters them against rules the user
set on their PC, and sends the survivors to a Tauri desktop app for low-attention
triage. Filtering happens **on the phone**, so a muted app's notifications are
never transmitted at all.

Two transports carry the same messages:

- **LAN** — the phone dials the desktop's TLS WebSocket listener directly, with
  the desktop's certificate pinned from the pairing QR. No account, no internet.
- **Relay** — when no local route exists, both devices dial a Cloudflare Worker
  on port 443 and meet there. The relay routes opaque frames; the payload is
  sealed in a device-to-device Noise session it holds no key for.

LAN is always tried first. The relay needs a verified Firebase account, purely to
authorise provisioning a pair.

---

## 2. Repository layout

```
focusbridge/
├── android/                  Kotlin phone app (Gradle, minSdk 26, compileSdk 34)
├── desktop/                  Tauri 2 desktop app
│   ├── src/                  React + TypeScript front end
│   ├── src-tauri/            Rust back end (the actual app)
│   └── core/                 focusbridge_core — logic shared by the app and its tests
├── shared/
│   ├── secure-channel/       The Noise engine, in Rust. One implementation.
│   ├── secure-channel-jni/   JNI wrapper + prebuilt .so for four Android ABIs
│   └── protocol.json         The wire contract, as data
├── relay-worker/             The deployed relay: Cloudflare Worker + Durable Object
├── relay/                    An older self-hosted Actix relay. NOT deployed, not used.
├── tools/cloudflare/         Pinned Wrangler, for deploying the worker
└── docs/                     Everything written down
```

**`relay/` is a dead end.** It is the original self-hosted Actix relay from the
build playbook. Cross-network sync does not use it and no client dials it. It is
still tested by `relay-ci` and still compiles; do not confuse it with
`relay-worker/`, which is the one that is live.

---

## 3. The desktop app (`desktop/`)

### 3.1 Rust back end — `desktop/src-tauri/src/`

| File | What it owns |
|---|---|
| `main.rs`, `lib.rs` | Startup. Installs the rustls crypto provider **before anything opens a connection** (rustls 0.23 turns a missing choice into a runtime panic on a worker thread — every test passed without it and the built app could not open TLS at all). Binds the LAN listener on `LOCAL_WS_PORT = 9173`. |
| `state.rs` | `AppState` — the single source of truth for the live connection. Holds `is_paused` (the manual disconnect), the vault lock, the pairing session, connection diagnostics, and the reconnect-request permit. **Rule R2 lives here.** |
| `server/ws_server.rs` | The LAN listener and, via the loopback bridge, every relay session too. Authentication, the attach decision, `MAX_UNAUTHENTICATED_CONNECTIONS = 24`. **This is where "may this phone attach?" is decided, for both transports.** |
| `server/handler.rs`, `socket_io.rs`, `heartbeat.rs`, `tls.rs` | Frame service loop, pending-message queue, the 3s ping / 6s timeout heartbeat, the self-signed certificate. |
| `sync/relay_client.rs` | Dials the Worker on 443, drives the Noise **responder** handshake, then bridges decrypted records into the local listener over a loopback connection pinned to this process's own certificate. |
| `sync/relay_api.rs` | Provisions and revokes relay pairs with a Firebase ID token. |
| `sync/relay_identity.rs` | The desktop's Noise static key, the per-pair PSK, and the pinned phone key — in the SQLCipher settings table. |
| `sync/protocol.rs` | Envelope types on the desktop side. |
| `commands/*.rs` | The Tauri command surface — the *only* way the front end touches the back end. See §3.3. |
| `db/encrypted.rs`, `store.rs`, `models.rs` | SQLCipher open/migrate/probe, and all queries. |
| `security/database_key.rs` | The database key, wrapped by Windows DPAPI. Never derived from a user password, so background sync survives a locked UI. |
| `pairing/qr_generator.rs`, `cert_manager.rs`, `device_store.rs` | The pairing code, the certificate, the saved devices and the persisted pairing session (`pairing.session.v1`). |
| `priority/`, `desktop_notifications.rs`, `tray/` | Ranking, Windows toasts, tray. |

`desktop/core/` (`focusbridge_core`) holds what both the app and its tests need
without dragging in Tauri: `qr.rs` (590 lines — the v3 compact encoder and the
JSON fallback), `protocol.rs`, `handler.rs`, `secure_envelope.rs`, `relay.rs`,
`priority.rs`, `study_mode.rs`, `cert.rs`.

### 3.2 Front end — `desktop/src/`

React + TypeScript + Zustand. `components/` (PairingQR, PreviousConnections,
CrossNetworkPanel, AppRulesPanel, SettingsPanel, NotificationList/Card, AuthGate,
ConnectionStatus, StudyModeToggle, FilterPanel, EmptyState, PasswordInput),
`stores/` (notification, connection, appRules, settings), `hooks/`, `lib/`
(`firebaseAuth`, `accountSession`, `relay`), `utils/` (connectionHealth,
lockTimeout, navigation, pairedDevices, priority, time).

The front end holds **no** authority. It cannot decide whether a phone may
attach, and it cannot unlock the vault by hiding a screen — see §6, bug 9.

### 3.3 Tauri commands

`app_rules`: `list_app_rules`, `set_app_rule` ·
`auth`: `auth_status`, `auth_register`, `auth_register_with_recovery`,
`auth_recovery_question`, `auth_reset_password_with_recovery`,
`auth_update_recovery`, `auth_login`, `auth_lock` ·
`diagnostics`: `get_connection_diagnostics` ·
`notification`: `list_notifications`, `mark_important`, `mark_ignored`,
`delete_notification`, `clear_notifications_older_than`,
`clear_notifications_between`, `clear_all_notifications` ·
`pairing`: `generate_pairing_qr`, `consume_pairing`, `list_paired_devices`,
`delete_paired_device`, `disconnect_phone`, `request_device_reconnect` ·
`relay`: `relay_set_auto_connect`, `relay_status`, `relay_set_url`,
`relay_enable`, `relay_disable` ·
`settings`: `get_settings`, `set_study_mode`, `set_rule_text`,
`set_lock_timeout_minutes` ·
`windows_setup`: `run_windows_first_run_setup`

Three relay-auth commands (`auth_relay_otp_start`, `auth_relay_otp_verify`,
`auth_google_sign_in`) were **removed** in `f34eb30`: they survived the deletion
of the panel that called them and would post credentials to any host handed to
them, plain `http` included. Removing a UI does not remove a registered command.

### 3.4 Desktop storage

SQLCipher, `migrations/001_initial.sql`: `notifications` (id, app_name,
package_name, sender, message, timestamp, received_at, status, priority,
content_hidden), `settings` (key/value — also where relay identity and the
pairing session live), `paired_devices` (device_id, pairing_key, mode, endpoint,
cert_fingerprint, is_active, last_connected_at), `app_rules` (package_name,
label, category, icon_data_url, muted, priority, study_safe, counters).

---

## 4. The phone app (`android/`)

Kotlin, Jetpack Compose, Hilt, Room, CameraX. `minSdk 26`, `compileSdk 34`,
`targetSdk 34`, JVM target 17.

| Package | What it owns |
|---|---|
| `service/NotificationService.kt` | The `NotificationListenerService`. The entry point for everything. |
| `service/SyncForegroundService.kt`, `BootReceiver.kt` | Keeping the connection alive, and surviving reboot. |
| `processor/` | `NotificationProcessor`, `NotificationFilter`, `NotificationParser`, `ParsedNotification`, `parsers/DefaultParser`. Filtering happens here, before anything is sent. |
| `priority/` | `PriorityEngine`, `UrgencyDetector`, `StudyModeManager`. |
| `sync/SyncEngine.kt` | Which endpoint to dial and when. LAN candidates first, then the relay. `CONNECT_TIMEOUT_MS = 4000`, `RELAY_CONNECT_TIMEOUT_MS = 12000`, `RECONNECT_INTERVAL_MS = 15000`. **Rule R1 lives in `autoReconnectEnabled()`.** |
| `sync/WebSocketClient.kt` | 726 lines. Both transports, the envelope unwrap, the approval prompt and its dedupe, `AUTH_OK` handling. |
| `sync/PinnedTls.kt` | Certificate pinning from the QR. |
| `sync/LocalNetworkProbe.kt` | Whether a LAN route exists at all — on mobile data the saved private addresses are not waited on. |
| `sync/Protocol.kt`, `SecureEnvelope.kt`, `ConnectionState.kt`, `RetryStrategy.kt`, `ConnectionHint.kt`, `AppInventoryProvider.kt` | Wire types and support. |
| `sync/secure/` | `NativeSecureChannel` (the JNI surface), `SecureChannelBridge`, `PhoneSecureSession`. The Noise **initiator**. |
| `pairing/` | `PairingManager`, `CertificateManager`, `PhoneIdentity`, `DeviceInfo`. |
| `security/` | `DatabaseKeyStore` (Android Keystore), `DeviceIdentityStore`/`Provider`, `MobileAppLockCrypto`, `MobileLockAttempts`, `MobileLockSession`. |
| `storage/` | `DatabasePreparation`, `EncryptedDatabaseMigrator`. |
| `data/local/`, `data/repository/` | Room: `FOCUSBRIDGE_SCHEMA_VERSION = 4`; entities Notification, Pairing, Config, AppRule. Schema 4 added the relay columns. |
| `jniLibs/` | `libfocusbridge_secure_channel_jni.so` for arm64-v8a, armeabi-v7a, x86_64, x86 — **committed on purpose**, so a CI-built APK is not silently broken. Regenerate with `shared/secure-channel-jni/tests/build-android.ps1 -Ndk <ndk>`. ProGuard keeps the native method names. |

Config keys that matter: `manual_disconnect`, `auto_reconnect`.

---

## 5. How the two devices actually connect

### 5.1 Pairing

1. The desktop generates a self-signed certificate and mints a pairing session
   (five-minute life), persisted as `pairing.session.v1`.
2. `generate_pairing_qr` produces the **v3 compact binary** payload: 407
   characters, readable from 300px up. The old JSON link (963 characters, 117
   modules, unscannable at panel size) is still the fallback for anything the
   compact form cannot carry, and pasted JSON still parses.
3. The payload carries the LAN candidates (routed address first — a Hyper-V
   virtual switch used to lead and cost a ten-second timeout every pairing), the
   certificate fingerprint, the pairing key, and — only as a complete set — the
   `relay` block (url, accountKey, pairId, phone capability) and the `noise`
   block (desktop public key, enrollment PSK). A partial block leaves the
   pairing LAN-only.
4. The phone scans, shows what it is about to connect to including the
   certificate code, and waits for the user. A `focusbridge://pair` deep link
   cannot pair anything on its own.

### 5.2 The attach decision — `server/ws_server.rs`

One function, for every transport. A phone may attach when:

- automatic reconnection is **on**; or
- it presents the pairing key of the code **currently on screen**; or
- the user asked for that specific phone by name (a single-use allowance, spent
  only if it was actually needed).

Anything else is refused, and a refusal sends the relay client to idle rather
than into a retry loop.

### 5.3 The relay path

```
phone ──TLS 443──▶ Cloudflare Worker ──▶ Durable Object (one per hashed account)
                          │
desktop ──TLS 443─────────┘
   └─ decrypted records ──▶ loopback WSS to its own listener on 9173
```

Worker routes (`relay-worker/src/index.ts`):
`GET /health`,
`/v1/socket/<64 hex>/<32 hex>/(desktop|phone)` (WebSocket),
`/v1/pairs[/<32 hex>]` (owner provisioning, Firebase ID token).

Deployed at `https://focusbridge-relay.focusbridge.workers.dev`, Workers Free,
`workers.dev` hostname (a business launch needs a custom domain). Frames are
opaque, capped at 64 KiB, with a persisted per-role token bucket, session
budgets, ten pairs per account, and a creation budget that survives pair
deletion so churn cannot escape it. No queue: if the peer is absent the relay
says so, and durable retry stays on the endpoints. It deliberately cannot read
content, manufacture an acknowledgement, or report a phone as authenticated.

### 5.4 Encryption

`Noise_XXpsk3_25519_ChaChaPoly_SHA256`, one Rust implementation
(`shared/secure-channel/`), reached from Android through JNI. The prologue binds
the protocol version and the pair ID, so a session cannot be replayed into
another pairing. Handles are opaque, process-local, capped, never reused, and
retired on any error — a session that has seen a bad frame can no longer tell
replay from loss.

Two implementations of one handshake are two things that can disagree, and a
disagreement in a handshake is a security bug. Hence one engine.

### 5.5 Message types

`AUTH`, `AUTH_OK`, `AUTH_FAILED`, `NOTIFICATION`, `NOTIFICATION_ACK`,
`NOTIFICATION_BATCH`, `DISMISSAL`, `DISMISSAL_BATCH`, `PING`, `PONG`, `STATUS`,
`APP_INVENTORY`, `RULES_UPDATE`, `RULES_ACK`, `DESKTOP_ACTION`, `UNPAIR`,
`ENCRYPTED` (`desktop/core/src/protocol.rs`, mirrored in
`android/.../sync/Protocol.kt` and `shared/protocol.json`).

`AUTH_OK` is sent **in the clear**; everything after it is wrapped in the
pairing-key envelope. Both transports share one unwrap — see §6, bug 6.

---

## 6. The bug ledger

Every defect that has bitten this project, what actually caused it, and what
holds it fixed. Ordered newest first within each area. The pattern worth
internalising: **almost every one of these was a rule enforced in two places
that could not both be right.**

### Connect / disconnect (the area that has broken most often)

1. **`cdf418f` — Disconnect lasted about seven seconds.** A phone keeps its
   pairing key, and the code on screen lives five minutes, so for those five
   minutes the just-disconnected phone still presented the key of the displayed
   code — the exact test for "has just scanned" — and was let straight back in.
   *Fix:* disconnecting retires the code on screen, so the next code is a
   genuinely new one. The flag was renamed to what it actually tests. *Hazard
   that came with it:* the saved device's certificate fingerprint was read from
   the pairing session, now cleared on disconnect, and would have been written
   back blank, breaking the pinning that reconnecting depends on; it is read
   from this PC's own certificate instead.
   *Held by:* `state::vault_lock_tests::disconnecting_retires_the_pairing_code_on_screen`.

2. **`9a61edb` — A valid code that nobody answered.** The disconnect was
   enforced in two places that could not agree: the pairing screen decided
   whether to resume, and the relay client refused to be present at all. So a
   code worked or did not depending on how it came to be drawn.
   *Fix:* showing a code never lifts a disconnect; the PC stays **findable** at
   the relay, and what may **attach** is decided in one place, at
   authentication. Being findable is not the same as accepting.

3. **`97fa4dd` / `3d0dfca` — Scanning after a disconnect did nothing.** Resuming
   was tied to minting a *new* session, and after a disconnect the previous code
   is usually still inside its five minutes, so the code was re-shown and nothing
   resumed. Also in `3d0dfca`: a refused authentication now says *why* — a phone
   whose credentials this PC no longer holds stops and asks to be scanned again,
   instead of retrying every fifteen seconds forever on mobile data.

4. **`785e368` — The reconnect setting did nothing over Wi-Fi.** The check asked
   `peer.ip().is_loopback()`, so it governed the relay and nothing at all over
   the LAN — which is the path a same-network phone takes first. Shipped broken
   twice. *Fix:* one decision function, asking nothing about how the phone
   arrived. *Held by:* `ws_server::attach_tests`.

5. **`edad4c6` — Notifications stopped arriving while both ends showed
   connected.** The send path checked the persisted `manual_disconnect` flag
   *before* checking whether a session was live, so a phone that had once been
   disconnected and had since reconnected dropped every notification at source.
   *Fix:* an authenticated session ends the disconnect, and a live session is
   the deciding fact. The whole path is now logged on both sides (no content).

6. **`fab5b75` — Every desktop reply dropped after authentication.** `AUTH_OK`
   is sent in the clear and everything after it is wrapped in the pairing-key
   envelope; the relay transport dispatched without unwrapping. Symptom: a
   reconnect every ~180s, exactly the heartbeat timeout. Covered by an
   **on-device** test, because `android.util.Base64` returns defaults under the
   JVM runner and would have let a broken implementation pass.

7. **`a69d960` — The reconnect button was dead.** The wake used
   `notify_waiters`, which only reaches an already-parked task; a request
   arriving in the gap between testing the flag and parking was lost while the
   flag stayed set. *Fix:* store a permit, plus a periodic recheck. Same commit:
   the relay bridge never answered the local server's transport probe, so quiet
   sessions died after ~2 minutes.

8. **`4501bf7` / `f170755` / `082e092` / `19a96fc` / `0dfa7d0` / `28f129b`** —
   the long sequence in which "who may reconnect" was moved out of presence, out
   of the pairing screen, out of the transport, and into one authenticated
   decision on each side. Each of these fixed a real report and broke a
   neighbouring one; that is why the behaviour checklist exists. The durable
   results: presence is not permission; a disconnected phone keeps a control-only
   presence at the relay so it can be asked back; a LAN-only pairing goes fully
   quiet, because there is no way to ask it anything.

### Security

9. **`35683ae` / `42ca81f` — The lock was interface-only.** It hid the dashboard
   and did nothing about backend desktop notifications, so a message arriving
   during PIN entry appeared in full to whoever was in front of the machine. The
   stored inbox was also readable by anyone who asked. *Fix:* the backend knows
   whether the vault is open; it starts locked, opens on a verified PIN, closes
   on idle timeout and sign-out. Listing is refused while locked and the event
   is not emitted. Nothing is lost — the inbox loads from the database on mount.

10. **`3d121b6` — Any app or web page could silently re-pair the phone.**
    `focusbridge://pair` was consumed with no confirmation, and the v2 payload
    carries relay credentials, so a malicious link could have streamed
    notifications to a stranger from any network.

11. **`f34eb30` — Dead commands that posted credentials anywhere.** See §3.3.

12. **`feafa33` — Unbounded connections to the LAN listener.** Bound to
    `0.0.0.0`; each socket costs a TLS handshake, buffers and a task until the
    auth deadline. Now 24 at a time, excess dropped rather than queued, since
    queueing moves exhaustion rather than refusing it. Measured: 200 idle
    sockets → 179 refusals, process steady at 45 MB.

13. **`ab9b7de` — A rejected database key rewrote the database.** SQLite
    checkpoints the WAL into the main file when a failed connection drops, and
    the frames are ciphertext so a wrong key does not stop it: a wrong-key
    attempt silently rewrote 8,164 bytes of a database its owner might still
    have been recovering. *Fix:* `SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE` until the
    key is proven.

14. **`ab9b7de` — The Android migrator refused every real migration.** It failed
    on a zero-length rollback journal, which SQLite defines as *not* hot. Now
    retired as cold, while anything holding data still fails closed.

15. **`edad4c6` — rustls 0.22 advisories, and the panic that followed.** Moving
    to rustls 0.23 requires the process to choose a TLS provider; not choosing
    is a runtime panic on a worker thread, not a compile error. Every test
    passed and the built app could not open TLS at all. Same commit: a CSP for
    the webview, an unused shell-open permission removed, Firebase gRPC/protobuf
    advisories pinned out, and release builds signed with a private key kept
    outside the repository.

16. **Cleartext traffic was permitted app-wide** though every transport is TLS.
    `usesCleartextTraffic` is now false, so a regression cannot downgrade one.

17. **Schema version was hard-coded in the storage layer,** so bumping Room to 4
    made the migrator reject the user's own database. Both now read
    `FOCUSBRIDGE_SCHEMA_VERSION`.

18. **`ef71f3b` — A chunking test that could not detect a leak.** It sealed a
    megabyte of one repeated byte and only checked the record after reassembly.
    Each chunk crosses the relay on its own, so each must hide its contents
    independently; it now uses an app-inventory-shaped payload and asserts none
    of those names appear in any frame.

### Pairing and QR

19. **`3d960f1` — The QR was unscannable.** 963 characters as percent-encoded
    JSON, a 117-module symbol, under three pixels a module at display size;
    decoding the rendered image confirmed it does not read at 300 or 360px.
    *Fix:* the v3 binary form, 407 characters, readable from 300px up.

20. **`19a96fc` — The code was drawn as a stretched rectangle.** Fixing both
    dimensions and then capping the width let the narrow panel squash it, and
    smooth scaling blurred the module edges. Now square at every width,
    nearest-neighbour.

21. **`fab5b75` — The in-app camera failed where the system camera succeeded.**
    The analyzer ran at the 640×480 default. Now 720p with `TRY_HARDER`.

22. **`413e146` — A reset or replaced phone could never pair again.** Enrollment
    was allowed only while no phone was pinned, and the pin was refused if it
    differed. A live pairing code now authorises replacing the pinned identity;
    outside that window nothing changes.

23. **`19cb02e` — A phone with no pinned key could never pair across networks.**
    Enrollment waited on a flag armed only by pressing for a fresh code on the
    main panel, so a reinstalled phone — or one that scanned the side panel's
    code — failed the relay handshake forever while LAN kept working. Enrollment
    now follows from the facts, still gated by the PSK from the code.

24. **`fab5b75` — Sync mode read a hard-coded `LOCAL` field,** so it claimed
    local sync while running over the relay. It now reports the transport the
    server actually recorded, and says nothing when disconnected.

25. **`fab5b75` — Pairing offered LAN addresses in numeric order,** putting a
    Hyper-V virtual switch ahead of the real adapter, so every pairing began by
    waiting out a ten-second timeout. The routed address now leads.

### Build and environment

26. **The desktop had not compiled for some time.** The tree had switched
    rusqlite to `bundled-sqlcipher-vendored-openssl`, which needs a native Perl
    this machine lacked, so no desktop test had run since. *Fix:* Strawberry
    Perl via scoop; use
    `PATH=~/scoop/apps/perl/current/perl/bin:$PATH` for desktop Cargo commands.

27. **Windows temp cleanup deleted the Android SDK from `C:\tmp`** — platform,
    build-tools, most of platform-tools, the system image and the AVD. The SDK
    now lives at `C:\Users\DSU\android-sdk` and the AVD at
    `C:\Users\DSU\focusbridge-storage-avd`. **Do not put tooling in `C:\tmp`.**

28. **`52d65a3` / `42ca81f` — The install script reported good installs as
    broken.** It checked for a string that lived in a test module and so is
    absent from release builds; it spliced parameters into a quoted string to
    elevate, so a `-Msi` path arrived with its quotes attached; it compared
    against the build directory, which cannot match because Rust builds are not
    reproducible byte for byte; and a path literal contained an unescaped tab.
    It now passes parameters as a file and compares the installed binary against
    the payload extracted from the installer that was actually run.

29. **Windows Installer will not overwrite a file in use and reports success
    anyway,** so uninstalling while FocusBridge was running left the previous
    build in place and every symptom pointed at the code. The script now refuses
    to continue if the app is running and checks the file actually disappeared.

---

## 7. Features, and how each was built

| Feature | Commit | How |
|---|---|---|
| Notification mirroring | `0dc78fe` and after | `NotificationListenerService` → filter → `NotificationProcessor` → `SyncEngine` → desktop store → toast |
| Rules enforced on the phone | `9ebc6fc`, `3a6eca9` | Desktop edits `app_rules`, pushes `RULES_UPDATE`; the phone applies them before sending, so muted apps never transmit |
| App inventory and icons | `b82b49e`, `d3b649a` | `AppInventoryProvider` sends the installed list with icons as data URLs; the desktop reconciles into `app_rules` |
| Study Mode | `e12f0ad` | `StudyModeManager` + `study_safe` per app |
| Masked Peek | `d5dc431` | `content_hidden` on the record; the body is revealed on hover or click only |
| Tray and keep-alive | `c991eee` | Tray menu, window-close to tray |
| Heartbeat | `adaa9a2` | 3s ping / 6s timeout, answered over the relay bridge too |
| Delivery acknowledgements | `888b403` | `NOTIFICATION_ACK` back to the phone |
| Diagnostics | `77ffbcd` | `get_connection_diagnostics` — transport, last heartbeat, last auth failure, last disconnect reason |
| Windows first-run setup | `5e28345` | `run_windows_first_run_setup` |
| Firebase account auth | `fc7e82d`, `7b0b966` | Used **only** to authorise relay provisioning |
| App lock with custom timeout | `87eabcf`, `35683ae` | Backend-owned vault state; idle timeout and sign-out close it |
| Encryption at rest, both platforms | `ab9b7de` | SQLCipher; key wrapped by DPAPI / Android Keystore, never from a password, so background sync survives a locked UI. Migration exports to staging, verifies through a fresh keyed open, fsyncs, then atomically renames — it never destroys the original and refuses rather than guessing |
| One Noise engine for both platforms | `5bd8256` | Rust + JNI; four ABIs committed into `jniLibs` |
| The free relay | `e0d92b8` | Worker + SQLite Durable Object per hashed account, Workers Free |
| Cross-network sync | `3d121b6` | Both ends dial 443; decrypted records bridged into the existing LAN listener over pinned loopback, so one tested path serves both transports |
| Per-side reconnect switch | `0dfa7d0` | Rules R1 and R2; see the behaviour checklist |
| Reconnect a known phone from the PC | `19a96fc`, `0dfa7d0` | `request_device_reconnect` joins the relay and waits, rather than failing with "phone is offline" |
| Compact v3 QR | `3d960f1` | Binary payload, 407 chars; JSON fallback retained |

---

## 8. Build, test and run

### Android
```
cd android
./gradlew test lint assembleDebug          # what CI runs
./gradlew testDebugUnitTest                # 114 JVM tests
```
Needs `local.properties` with `sdk.dir` (git-ignored; here it is
`C:/Users/DSU/android-sdk`). Release signing comes from
`FOCUSBRIDGE_KEYSTORE_PROPERTIES` or
`../../FocusBridge-signing/keystore.properties`; absent, the build falls back to
the debug key and is for testing only.

Instrumented tests need the disposable emulator:
`android/app/src/androidTest/run-storage-proof.ps1` (AVD
`focusbridge-storage-proof-api34`, at `C:\Users\DSU\focusbridge-storage-avd`).

### Desktop
```
cd desktop
pnpm install --frozen-lockfile
pnpm tsc --noEmit && pnpm vitest run && pnpm build
cargo test --locked                        # needs Perl on PATH, see §6.26
cargo clippy --locked -- -D warnings
```
`tests/encrypted_database.rs` needs an independent SQLite: set
`FOCUSBRIDGE_SQLITE3_TEST_BIN` to a real `sqlite3.exe` or to
`desktop/scripts/ordinary-sqlite3.cmd` (CPython's own SQLite).

### Relay worker
```
cd relay-worker && npm test && npx tsc --noEmit
cd ../tools/cloudflare && npx wrangler deploy      # pinned Wrangler 4.129.0
```

### Native crypto
```
cd shared/secure-channel && cargo test
pwsh shared/secure-channel-jni/tests/build-android.ps1 -Ndk <ndk path>
```

### Rough test counts at `cdf418f`
Android JVM 114 · Android instrumented 34 · desktop Rust 81 · frontend 49 ·
relay worker 65 · shared crypto 17.

---

## 9. CI

`.github/workflows/`:

| Workflow | Runs on | Does |
|---|---|---|
| `android-ci.yml` | ubuntu-22.04 | `./gradlew test lint assembleDebug` |
| `desktop-ci.yml` | ubuntu-22.04, macos-latest, windows-latest | tsc, vitest, build, `cargo check --locked` |
| `relay-ci.yml` | ubuntu-22.04 | fmt, clippy `-D warnings`, `cargo test`, docker build (against `relay/`, the unused one) |

### Known problem: android-ci is nondeterministic (open, 2026-09-07)

`android-ci` fails on roughly half of all runs, and **it is not a code
regression.** Proof: the `android/` tree object is byte-identical across
`e9a90d2`, `97fa4dd`, `9a61edb` and `cdf418f` (`dcc27a86…`), and that identical
tree both passed and failed. The same holds for tree `546b942b…` (`35683ae`
passed, `52d65a3` and `edad4c6` failed) and `6f5e1396…` (`785e368` passed,
`42ca81f` failed).

Where it fails: step 5, `./gradlew test lint assembleDebug`, after 100–170
seconds — i.e. late, in `test` or `lint`, not in dependency resolution. The
workflow file has not changed across any of these runs.

What has been ruled out locally: the same command passed 11 consecutive times on
Windows — 6 idle runs and 5 more under a 12-way CPU load, all with
`--rerun-tasks`. So it does not reproduce on a fast machine.

Remaining hypotheses, in order:
1. A flaky test that only loses its race on a 2-core runner. The real-time
   candidates are `SyncEngineTest` (`withTimeout(2_000)` around
   `connectActivePairing`, and `connectingAttemptKeepsTimeoutBeforeFallback`,
   which asserts ≥4s elapsed inside a 6s budget), `WebSocketClientTest` (several
   `CountDownLatch.await(2, SECONDS)`), and `MobileLockAttemptsTest`.
2. Memory pressure. `org.gradle.jvmargs=-Xmx2048m` plus the Kotlin daemon, KSP,
   lint and a test JVM on a 2-core / 7 GB runner.
3. A corrupted or racing Gradle cache restored by `gradle/actions/setup-gradle@v6`.

**Next step:** read the failing step's log — it names the task and the test in
one line and collapses this list to one item. `gh` is not installed on this
machine and the Actions log API refuses unauthenticated requests, so it needs
either `gh auth login` or a copy-paste from the run page. A worthwhile companion
change is to add `--stacktrace` and an `actions/upload-artifact@v4` of
`android/app/build/reports/` and `android/app/build/test-results/` on failure,
so the next failure is diagnosable without a person watching.

---

## 10. What is not done

- **No independent security audit.** The reviews in `docs/` are self-reviews.
- **No overnight endurance run.**
- **Notification actions** — you can see a notification but not finish with it,
  so the alert is still waiting on the phone. This is the one real gap against
  Phone Link (`docs/phone-link-gap-analysis.md`); the other differences are
  deliberate scope choices.
- **Device matrix** — verified on one Pixel 7 and one Windows PC.
- **`workers.dev` hostname** — Cloudflare documents it as personal/hobby use; a
  business launch needs a custom domain and a route.
- **`relay/`** is unused and could be deleted, along with a third of `relay-ci`.

---

## 11. Working rules on this project

1. **Walk `docs/behaviour-checklist.md` before every commit.** Fixes here have
   repeatedly undone one another, and each of those reached the user before it
   reached a test.
2. **Add a checklist row for every fixed bug and every new feature**, naming the
   test that holds it. If two rows conflict, that is a design question to settle
   out loud, not to resolve quietly in code.
3. **Do not commit or push work that has not met the full production release
   gate.** Automated test success alone does not satisfy it.
4. **Never describe untested behaviour as working.** Say what was verified, on
   what hardware, and what was not.
5. **Free-first infrastructure.** No paid resources or auto-charging trials
   without explicit approval.
6. When a rule seems to be enforced in two places, it is a bug in waiting. Find
   the one place it belongs.
