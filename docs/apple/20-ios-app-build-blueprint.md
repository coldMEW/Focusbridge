# 20: FocusBridge for iPhone — build blueprint

Date: 2026-09-08. Baseline `cdf418f`. **Documentation only.** Rollback is deletion of
this file and its index link in report 15. Read `19-verified-capture-routes-and-architecture.md`
first; this document assumes its verdicts and does not re-argue them.

This is a build plan, not a proof. Section 1 states the single gate that must pass
before any iOS code is written, and section 12 states what changes if it fails.

---

## 1. The gate: run X1 before writing code

Everything below assumes the iOS 27 notification automation passes the notification's
**content** into the shortcut. That is the one load-bearing fact still unverified on
hardware. It is cheap to settle and it can be settled today.

**X1 is runnable now, free, in about 30 minutes.** iOS 27 **public** beta 6 has been
out since 2026-08-31, so no paid developer account is needed to test.

1. Put iOS 27 public beta on a spare iPhone (Settings → General → Software Update →
   Beta Updates, after enrolling at beta.apple.com).
2. Install **ntfy** (free, open source, App Store). Subscribe to a random topic.
3. From the PC: `curl -H "Title: FB-TITLE-A1" -d "FB-BODY-B2" ntfy.sh/<your-topic>`
   — this delivers a real third-party notification with a title and body you control.
4. In Shortcuts → Automation → New → **Notification**, pick ntfy, **no filters**
   (filters were broken in early betas), Run Immediately, Notify When Run **off**.
5. Action: `Text` → insert variable → look for **Shortcut Input** and any
   notification variable. Then `Save File` / `Append to Text File` so you can read
   the result. Do not use `Show Notification` — it can retrigger the automation.

**Record exactly which of these are retrievable, separately:** source app, title,
subtitle, body, delivery date, any identifier. Then repeat locked, with Show
Previews = Never, with a Focus on, in Low Power Mode, after a reboot, and with a
burst of 20 published in 5 seconds.

| Outcome | Consequence |
|---|---|
| Title, subtitle and body all available as distinct variables | Build everything below as written |
| One concatenated blob only | Build it, but ship parsing as best-effort and never claim structured sender/subject |
| Trigger only, no content | **Stop.** Route A cannot mirror notifications. Fall back to §12 |

Nothing else in this blueprint is blocked on Apple. X1 is.

### 1.1 UPDATE, same day: the evidence has turned against content-passing

A second, deeper research pass changed the odds substantially. **Plan for "trigger only."**
Nothing below is proven, but the weight of evidence has shifted and this blueprint's
optimism in §2–§10 must be read against it.

1. **Apple's full WWDC26 session 310 transcript describes filtering and nothing else.**
   The only passage on this feature: *"The notification automation is triggered when I
   receive a notification from the Soup Chef app. I don't want it running for every
   notification from the app so I've added a filter for the word 'arriving.'"* Across the
   whole session Apple never mentions a variable, Shortcut Input, or any notification
   property — in a session that *is* explicit about data flow for Storage and App Entities.
2. **Apple's own demo argues against it.** The demo notification contains a driver's name
   and an arrival time, yet the resulting HomePod announcement is a hardcoded
   *"Your soup is almost here."* If the name were readable, that is the demo you'd give.
3. **Two hands-on reports say it outright.** On the Homey community forum:
   *"the shortcut can't read the content of the notification"*, and
   *"The information isn't extracted from any other apps even Apple's own messages app."*
   — https://community.homey.app/t/ios-27-notifications-and-new-shortcuts/156044
4. **The most technical published coverage is filter-only throughout.** Derek Seaman's two
   long posts describe *"trigger actions based on"* title/subtitle/message — never *use in* —
   and his example shortcut only enables a Focus mode. No variable appears anywhere.
5. **Apple has published no iOS 27 Shortcuts user-guide page for it.** The 27.0 guide URL
   still serves iOS 26 content, and the trigger list remains Event / Travel / Communication
   / Transaction / Setting.
6. **There is no App Intents or UserNotifications API surface for it** — verified by
   absence across the DocC index.

The one piece of wording that cuts the other way is Apple's *"These details make the
notification easy to parse and interpret within a shortcut."* In context it is advice to
app developers about writing filterable notification copy, and it sits between two
sentences about the keyword filter. **Do not build on that sentence.**

### 1.2 This also puts Tier 2 in doubt — and that is a correction to my own earlier claim

I previously wrote, in report 19 §5 and in §2 below, that the shipping **Message** and
**Email** triggers pass content — the body as Shortcut Input and the sender as a `Sender`
variable. **That is now contested and must be treated as unverified.**

Apple's own documentation describes both purely as filters:
*"Use a communication trigger to run an automation when you receive an email or message…
When you add multiple criteria… all criteria must be met."* The options are `Sender`,
`Subject Contains`, `Account`, `Recipient` for email, and `Sender`, `Message Contains` for
messages — every one worded as *"Triggers your automation when you receive…"*
— https://support.apple.com/guide/shortcuts/communication-triggers-apdd711f9dff/ios

Against that: widely-used community SMS-forwarding shortcuts exist and appear to move
message text, and a published walkthrough describes assigning a `Sender` variable. But a
commenter on that same walkthrough reports *"I cannot find the options to forward the body
of the text message."*

**So Tier 2 needs its own test (X6) and must not be assumed.** If the communication
triggers are filter-only too, then the structural parallel is complete: Shortcuts
automation triggers on iOS are a *matching* mechanism, not a *data source*, and Route A
cannot mirror notifications at all.

### 1.3 The two-minute version of X1

Before the fuller X1 protocol above, do this first — it converts the central unknown into
a fact in about two minutes:

> Create the notification automation. Make its **only** action `Show Result`. Open the
> variable picker. See whether anything notification-shaped is offered above the action
> list.

If the picker is empty, stop and go to §12.

---

## 2. What we are building

A native iOS app, **FocusBridge for iPhone**, that is a *notification source* peer for
the existing desktop app. It is not a port of the Android app; the phone-side lifecycle
is fundamentally different and pretending otherwise is the main way this goes wrong.

**Three capture tiers**, presented honestly in the UI, never merged into one claim:

| Tier | Mechanism | iOS | Coverage |
|---|---|---|---|
| **1. App notifications** | Shortcuts notification automation → App Intent | 27+ | Any app, one automation per app |
| **2. Messages & Mail** | Shortcuts Message/Email triggers → App Intent | 13+ | SMS/iMessage/Mail only |
| **3. Manual share** | Share Sheet extension | 13+ | Anything the user shares, labelled manual |

Tier 2 ships first because it works on every current iPhone and needs no beta. Tier 1
is the headline once X1 passes.

**Explicitly out of scope for v1:** iPhone as a *receiver*, ANCS (that is a desktop-side
project, report 19 §4), notification dismissal sync back to the phone, replying to the
originating app, and app icons. None of these are achievable on this route; §9 says so
in the product surface, not just in the docs.

---

## 3. Architecture

The Android model is a foreground service holding a persistent socket. **iOS cannot do
that and no amount of engineering changes it.** The iOS model is *episodic*: nothing
runs until a notification arrives, then a short-lived process does everything and exits.

```
   notification arrives
          │
          ▼
  Shortcuts automation  (user-created, per app, Run Immediately, locked-safe)
          │  passes title / subtitle / body / app
          ▼
  ┌──────────────────────────────────────────────────────────────┐
  │ ImportNotificationIntent : AppIntent                          │
  │   supportedModes = .background     (iOS 26+; NOT openAppWhenRun)│
  │   authenticationPolicy = .alwaysAllowed   (default; runs locked)│
  │   no dialog, no requestDisambiguation, no UI                   │
  │                                                                │
  │  1. validate + bound input (see §7.1)                          │
  │  2. evaluate cached RulesSnapshot  → drop / mask / prioritise  │
  │  3. append to encrypted outbox  ← durability point             │
  │  4. try deliver now (budget ~20s of the 30s)                   │
  │  5. mark delivered only on NOTIFICATION_ACK                    │
  └───────────────────────────┬──────────────────────────────────┘
                              │
        ┌─────────────────────┴─────────────────────┐
        ▼                                           ▼
  LAN (preferred)                             Relay (fallback)
  Network.framework                           URLSessionWebSocketTask
  NWConnection + NWProtocolTLS                wss://…workers.dev/v1/socket/…
  pin SHA-256(DER) from QR                    Authorization: Bearer <cap>
  + NWProtocolWebSocket                       binary Noise frames
  → AES-GCM ENCRYPTED envelope                → Noise_XXpsk3 via Rust crate
        └─────────────────────┬─────────────────────┘
                              ▼
                    existing Tauri desktop
                  (Windows today, macOS per report 11)
```

**Retry path when the intent expires:** the outbox survives. The next intent run drains
it first. Backstops in priority order: (a) a `BGAppRefreshTask` (unreliable, best
effort), (b) drain on app foreground, (c) optional silent-push nudge (§10.4). We never
promise the OS will run us on demand.

---

## 4. Stack decisions, with the reason

| Concern | Choice | Why not the alternative |
|---|---|---|
| Language / UI | **Swift 6, SwiftUI**, iOS 17.0 deployment target | Tier 2 works from iOS 13, but SwiftUI + App Intents ergonomics below 17 cost more than the users gained. Tier 1 needs 27 anyway. |
| Capture surface | **App Intents** (`AppIntent` + `AppShortcutsProvider`) | The only sanctioned way a Shortcut hands data to an app. SiriKit intents are legacy. |
| Intent hosting | **In-app**, not an App Intents Extension, for v1 | The extension has a tighter memory budget and complicates Keychain/app-group and background-URLSession ownership. Revisit if launch latency disappoints. |
| Noise session | **Reuse `shared/secure-channel` Rust crate** via a C-ABI static XCFramework | Reimplementing `Noise_XXpsk3_25519_ChaChaPoly_SHA256` + the record framing in Swift would fork the security-critical code three ways. The Android JNI shim already proves an opaque-handle C API is sufficient. |
| Rust→Swift binding | **Hand-written C header, with cbindgen used in *verify* mode** (CI diffs generated output against the committed header) | Confirmed against UniFFI 0.32 and swift-bridge 0.1.59. UniFFI is mature but pre-1.0, does not remove any of the XCFramework work, and its `Arc<Mutex<T>>` object model would force abandoning the capped handle registry — which is a security property, not an implementation detail. A hand-written shim is a near-line-for-line transliteration of `bridge.rs`, so a reviewer can diff the two boundaries. Verify-mode cbindgen gives drift protection without build-time codegen. Revisit UniFFI only if `focusbridge-core` later crosses the same boundary. |
| Rust packaging | **`.xcframework`** built from `staticlib` for `aarch64-apple-ios` + `aarch64-apple-ios-sim` | A `lipo`'d fat static library mixing device and simulator slices is rejected by modern Xcode. XCFramework is the supported container. |
| LAN transport | **Network.framework** — `NWConnection`, `NWProtocolTLS` with `sec_protocol_options_set_verify_block`, `NWProtocolWebSocket` | The desktop uses a self-signed cert on a raw IP. ATS governs the URL Loading System, so `URLSessionWebSocketTask` would need ATS exceptions and a trust override that App Review scrutinises. Network.framework sits outside ATS and lets us pin the exact SHA-256 of the DER the QR carried. |
| Relay transport | **`URLSessionWebSocketTask`** | Public TLS to `*.workers.dev`, ATS-clean, and it can set the `Authorization: Bearer` header the Worker requires. No pinning needed. |
| Local store | **GRDB 7.10+ with SQLCipher via SwiftPM** | **Corrected — my first answer here was wrong.** App-layer AES-GCM per row leaves the schema, indices, row counts and the WAL in plaintext and forces per-row nonce management, where GCM nonce reuse under one key is catastrophic. SQLCipher encrypts pages, so metadata is covered. The licence is not a problem (Community Edition is BSD-style) and it buys nothing on export compliance, because the Rust Noise crate already puts us in the non-exempt bucket. Use GRDB+SPM, **not** rusqlite's `bundled-sqlcipher-vendored-openssl` — cross-compiling vendored OpenSSL to iOS is the genuinely risky path, and reports 03/08 already flag it unverified. |
| Key storage | **Keychain**, `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` | Must be readable while locked (§7.2). `ThisDeviceOnly` keeps identity keys off iCloud backups, matching the Android `noBackupFilesDir` decision. |
| QR scanning | **VisionKit `DataScannerViewController`** | Decodes the existing `focusbridge://pair?c=<b64url>` compact link. |
| Tests | XCTest + Swift Testing; **golden vectors shared with Rust/Kotlin** | §11. |

---

## 5. Module map

```
ios/
  FocusBridge.xcodeproj
  FocusBridge/
    App/                 FocusBridgeApp.swift, RootView.swift
    Intents/
      ImportNotificationIntent.swift      ← tier 1 + 2 ingest
      ImportMessageIntent.swift           ← thin wrapper, distinct params
      FocusBridgeShortcuts.swift          ← AppShortcutsProvider
    Capture/
      NotificationRecord.swift            ← the typed event (§6.1)
      RuleEngine.swift                    ← on-phone filtering, mirrors Kotlin
      RulesSnapshot.swift                 ← versioned, cached
      Dedup.swift                         ← §7.4
    Store/
      Database.swift                      GRDB, migrations, protection class
      Outbox.swift                        enqueue / drain / ack / expire
      KeyStore.swift                      Keychain wrappers
    Net/
      Pairing.swift                       compact-v3 QR decode (§6.3)
      LanTransport.swift                  Network.framework + pinning
      RelayTransport.swift                URLSessionWebSocketTask + Noise
      SecureChannel.swift                 Swift facade over the Rust C ABI
      Envelope.swift                      Codable wire types (§6.2)
      AesGcmEnvelope.swift                HKDF + AES-GCM (CryptoKit)
      DeliveryCoordinator.swift           transport choice, budget, retry
    Onboarding/
      LocalNetworkPermission.swift        §7.3
      AutomationSetupGuide.swift          §8
    Settings/ …
  FocusBridgeShare/                       share-sheet extension (tier 3)
rust/
  secure-channel-ffi/                     new crate: C ABI over shared/secure-channel
    src/lib.rs                            mirrors shared/secure-channel-jni/src/bridge.rs
    include/focusbridge_secure_channel.h  cbindgen output, committed
  build-ios.sh                            → FocusBridgeSecureChannel.xcframework
```

`rust/secure-channel-ffi` is a **new sibling** of the existing JNI crate, not a rewrite
of it. `shared/secure-channel` itself is not modified.

---

## 6. Protocol conformance — exactly what iOS must emit

Read from the code at `cdf418f`. Deviating from any of this breaks an existing client.

### 6.1 The notification record

Wire shape, from `android/.../sync/Protocol.kt:116-133` and consumed by
`desktop/src-tauri/src/db/store.rs:481-501`:

```json
{ "version": 1, "type": "NOTIFICATION", "payload": {
    "id": "content:<24 hex>",        // string, required in practice
    "appName": "Signal",              // string
    "packageName": "org.thoughtcrime.securesms",  // string; join key for app rules
    "sender": "Alice" | null,         // key always present, value nullable
    "message": "see you at 6" | null, // key always present, value nullable
    "timestamp": 1757340000000,       // epoch MILLISECONDS
    "priority": "LOW"|"NORMAL"|"HIGH"|"URGENT",
    "contentHidden": false,
    "batchId": "…"                    // OMITTED when absent, not null
}}
```

iOS-specific mappings and the honesty they require:

- `packageName` — iOS gives no bundle identifier for another app. Use a stable
  synthesised key `ios:<slug(sourceName)>` and **document it**. It is the join key for
  every app rule, so it must be stable across runs; derive it from the app display
  name with a fixed normalisation, never from a hash of content.
- `sender` — only meaningful for tier 2 (the Message trigger's `Sender` variable) and
  for tier 1 if X1 shows subtitle carries it. Otherwise `null`. **Do not invent it.**
- `priority` — computed **on the phone** by `RuleEngine`, exactly as
  `NotificationProcessor.kt:104-108` does. The desktop only maps the string to a score
  (`store.rs:678-687`), so an unknown string silently becomes 30. Emit only the four
  documented values.
- `id` — see §7.4.

### 6.2 Messages iOS must implement

Emit: `AUTH`, `NOTIFICATION`, `NOTIFICATION_BATCH`, `PING`, `STATUS`, `RULES_ACK`, `UNPAIR`.
Consume: `AUTH_OK`, `AUTH_FAILED`, `NOTIFICATION_ACK`, `PONG`, `RULES_UPDATE`, `DESKTOP_ACTION`.
Not applicable: `DISMISSAL`, `DISMISSAL_BATCH` (no removal signal exists on this route),
`APP_INVENTORY` (iOS cannot enumerate installed apps — see §9).

`AUTH` payload: `{pairingKey, deviceId, deviceName, phoneInstallId, role:"phone"}`.

**`NOTIFICATION_BATCH` is the one that matters and Android never sends it.** The desktop
already handles it (`ws_server.rs:370-379`) with payload `{"notifications":[ …records… ]}`,
expanding it into individual stores. iOS must use it under burst — see §7.5.

### 6.3 Pairing

Scan `focusbridge://pair?c=<base64url>`; decode the **compact v3** layout documented at
`desktop/core/src/qr.rs:108-117`:

```
byte 0      version = 3
byte 1      flags; bit0 = relay+noise block present
2..18       deviceId, raw UUID (16 bytes)
18..50      pairingKey (32 bytes)
50..82      certFingerprint, SHA-256 of the desktop cert DER (32 bytes)
82          candidate count N, then N × (4-byte IPv4 + 2-byte port BE)
if bit0:    1-byte relay URL length, URL bytes,
            accountKey(32) pairId(16) capability(32) desktopKey(32) psk(32)
```

Fall back to `focusbridge://pair?payload=<percent-encoded JSON>` (`QrPayload`). **The
relay and noise blocks are all-or-nothing**: a half-populated pair must degrade to
LAN-only, never dial a relay it cannot authenticate to (`qr.rs:137-142`).

Hex fields (`pairingKey`, `certFingerprint`, `accountKey`, `pairId`) are hex on the
JSON path and raw bytes in compact; base64 fields (`capability`, `desktopKey`, `psk`)
are base64 in JSON, raw in compact. Get this wrong and pairing silently fails.

### 6.4 LAN framing

Connect `wss://<ip>:9173`, pin `SHA-256(leaf DER) == certFingerprint`. Then every
application message is wrapped (`desktop/core/src/secure_envelope.rs`):

```
key        = HKDF-SHA256(salt: "focusbridge-v1", ikm: pairingKey_utf8,
                         info: "FocusBridge message encryption v1", L: 32)
envelope   = {"version":1,"type":"ENCRYPTED","payload":{
                "alg":"AES-256-GCM",
                "nonce": base64(12 random bytes),
                "ciphertext": base64(AES-256-GCM(plaintext_envelope_json))}}
```

CryptoKit covers both: `HKDF<SHA256>.deriveKey` and `AES.GCM`. Note the nonce is
base64 **standard** (with padding), not URL-safe.

This layer has no forward secrecy and no replay counter — it is the pairing key inside
pinned TLS. Do not describe it to users the way the relay path is described.

### 6.5 Relay framing

```
URL   wss://<host>/v1/socket/<accountKey:64 hex>/<pairId:32 hex>/phone
      no query string permitted — the Worker 404s on any search string
hdr   Authorization: Bearer <capability>   (43 chars, base64url alphabet)
```

Control frames from the Worker are **text**, ≤512 bytes, and are metadata only —
`{"type":"relay.peer_ready","generation":…}` and `{"type":"relay.peer_unavailable"}`.
Mirror `Protocol.kt:198-209`: never parse a text frame as peer data.

Peer traffic is **binary** Noise frames. Session parameters, from
`shared/secure-channel/src/lib.rs`:

- Profile `Noise_XXpsk3_25519_ChaChaPoly_SHA256`; PSK set at **index 3**.
- Prologue `"FocusBridge/session/v2/notification-sync/phone-to-desktop/"` ++ pairId (16 raw bytes).
- **Phone is the initiator**, desktop the responder. Handshake messages carry empty
  payloads and are capped at 256 bytes.
- After the handshake, a **mutual confirmation** before the session is usable: desktop
  sends `"FocusBridge/v2/desktop-ready" || handshake_hash`; phone replies
  `"FocusBridge/v2/phone-ready" || handshake_hash`. Skipping this hangs the session.
- Pin the desktop static: compare `get_remote_static()` against `desktopKey` from the
  QR; mismatch is a permanent authentication failure, never a retry.
- Record framing (`records.rs`): 60 KiB chunks, each prefixed by a 16-byte header —
  `record_counter u64 BE`, `total_len u32 BE`, `offset u32 BE` — each chunk sent as one
  Noise transport message. Out-of-order or mismatched offsets are rejected; a partial
  record expires after 30 s.
- Limits: 1,000,000 messages, 1 GiB, 24 h session age, 120 s handshake age. **Any
  protocol error permanently closes the session** — reconnect, never resume.

---

## 7. Platform constraints that will break the build if ignored

These are the traps. Each is sourced; each has a required mitigation.

### 7.1 The intent runs locked — and that is the default, but it must not open the app

Apple, on `AppIntent.authenticationPolicy`: *"The default value of this property is
[`.alwaysAllowed`], which allows the intent to run without authentication, **including
when the device is locked**."* So do nothing and we get locked execution.

But **`openAppWhenRun` is deprecated** (Apple's own doc says so, and "Setting this
property to true generates an error if the app intent runs in an app extension"). The
modern control is `static var supportedModes: IntentModes` (iOS 26+). Use:

```swift
static let supportedModes: IntentModes = .background
```

Never `.foreground`, never `requestDisambiguation`, never a `dialog`. The one
well-documented lock-screen App Intent hang is exactly the interactive case
(developer.apple.com/forums/thread/714313).

### 7.2 Keys must be readable while locked

Default Keychain accessibility is `kSecAttrAccessibleWhenUnlocked` — the intent will run
and then fail to read its own identity key. Required:

- Keychain items: `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`.
- Database file: `NSFileProtectionCompleteUntilFirstUserAuthentication`. **Correction:
  this is already the default** — Apple states it "is the default class for all
  third-party app data not otherwise assigned to a Data Protection class." So the work
  is not to *set* it but to ensure nothing **downgrades** it, and to set it explicitly on
  the App Group container directory rather than relying on inheritance across processes.
  Never use `.complete` for the database or session state, or a background run on a
  locked device cannot read its own files.
- Consequence to accept: after a **reboot with no unlock**, nothing works. Notifications
  captured in that window are lost, because we cannot even open the outbox. Document it;
  do not pretend otherwise.

### 7.3 Local network permission cannot be obtained in the background — this is the big one

From Apple's TN3179 *Understanding local network privacy*:

- *"Making an outgoing TCP connection [to a local network address] — Required: yes."*
  So `wss://192.168.x.x:9173` needs the Local Network privilege.
- *"There's no API to explicitly bring up the local network alert (FB8711182)"* — and
  none to read the current state.
- Decisively: for a process assumed to be running in the background, *"If such an
  extension performs a local network operation while its Local Network privilege is
  undetermined, **the system denies that operation** as it would for an iOS app running
  in the background."*

**Therefore:**
1. Onboarding must trigger and obtain the alert **in the foreground**, before any intent
   ever tries LAN. Apple supplies the technique — connect a UDP socket to a link-local
   IPv6 address, which raises the alert without sending traffic (`triggerLocalNetworkPrivacyAlert()`
   in TN3179). Do this on the pairing screen, with an explanation.
2. The intent must treat a LAN failure as *"skip to relay"*, immediately and without
   retry, because in the undetermined case it will never succeed and will only burn the
   30-second budget.
3. Persist a locally-observed "LAN has worked at least once" flag; if it has never
   worked, do not attempt LAN from an intent at all.
4. **The Simulator does not implement local network privacy.** All of this must be
   tested on a real device or it will look fine and ship broken.

### 7.4 Deduplication without a stable identifier

Android derives `id` as `content:<sha256[..24]>` over package+sender+message+timestamp+index
(`NotificationProcessor.kt:71-88`). iOS has no notification UID on this route, so we do
the same — and inherit the same flaw, which report 16 correctly flagged: **two genuinely
identical messages collide.**

Mitigation, and the limits stated in the UI:
- Include a monotonic per-device sequence in the hash input so identical text at
  different times stays distinct.
- Keep a bounded recent-hash window (say 200 entries / 10 minutes) to suppress a true
  double-delivery of the *same* automation firing twice.
- Never merge across a process restart on text alone.
- Ship a "duplicate handling is best effort on iPhone" line in the source's capability
  description, not buried in a help page.

### 7.5 The relay will disconnect you for bursting — batch or die

From `relay-worker/src/account.ts:231-238`, with the code's own comment: *"One token per
frame, 100-token burst, one token replenished per 600ms."* Exceeding it does not apply
backpressure — it calls `retire(pairId, 4008)` and **kills the pair's session**.

Every Noise record chunk is one frame, so a 200 KiB record costs 4 tokens.

**Required client behaviour:**
- Coalesce anything queued within the same intent run into a single `NOTIFICATION_BATCH`.
- Rate-limit outgoing frames to ≤ 1 per 600 ms sustained, with a client-side burst
  allowance strictly below 100.
- On close code `4008`, treat the pair as rate-limited: back off, do not reconnect
  immediately, and surface it as a real state rather than a generic disconnect.
- Note the desktop ACKs each item in a batch individually, so a 100-item batch produces
  100 desktop→phone frames. That spends the *desktop's* budget. Consider proposing a
  batched ACK; until then, cap batch size at ~40.

### 7.6 No persistent socket, and no WebSocket in a background session

`URLSessionConfiguration.background(withIdentifier:)` is documented as *"allows HTTP and
HTTPS **uploads or downloads**"* — WebSocket tasks are not supported there. So there is
no way to hold the relay socket open while suspended.

Also from the same page, a constraint people forget: *"If the user terminates the app
from the multitasking screen, the system cancels all of the session's background
transfers… the system does not automatically relaunch apps that were force quit."*

**Design consequence:** connection is per-episode. The only durable delivery primitive is
the outbox plus the next intent run. If we later add a background-URLSession fallback it
must be an HTTP POST to a *new* authenticated relay endpoint, and that endpoint does not
exist today — do not assume it.

### 7.7 Session reuse across intent runs

A fresh Noise XX handshake plus TLS per notification is expensive and burns tokens. Keep
a process-lifetime session cache: if the app process is still warm from the previous
intent (common during a burst), reuse the open session. Discard on any protocol error
(the crate closes permanently anyway), on 24 h age, and on process restart. Never
persist ciphertext or transport state across launches — re-handshake instead.

### 7.8 Feedback loops

The automation triggers on notifications. FocusBridge's own alerts and Shortcuts'
"Notify When Run" banner are notifications. The setup guide must state: never select
FocusBridge or Shortcuts as an automation source, and leave "Notify When Run" off. Add a
runtime guard that drops any record whose source resolves to our own bundle.

---

## 8. Onboarding: the per-app automation problem

An app **cannot create a Shortcuts automation programmatically.** It can offer a
*shortcut* to add (iCloud share link / `.shortcut` file), but the automation trigger is
user-created. There is no API and no entitlement for it.

So the honest flow is:

1. Pair by QR (camera).
2. Grant local network in the foreground (§7.3), with a one-line reason.
3. **Guided automation setup**: for each app the user picks from a list of apps they say
   they care about, show a numbered walkthrough with screenshots and a "Copy action name"
   button, then a **live verification step** — the app waits for the first record to
   arrive from that source and shows a green tick when it does. Verification is what makes
   this bearable; a wall of instructions with no confirmation is not.
4. Surface a persistent **Capture health** list: one row per configured source with
   "last seen" and a warning when a source has gone quiet for longer than its normal
   cadence. If a user deletes an automation, we must say so — not show green.

Do not claim a single switch. Do not silently install anything.

---

## 9. Feature parity — what actually ships

| Existing feature | iOS v1 | Note |
|---|---|---|
| Notification capture | **Yes, per-app** | Tier 1 needs iOS 27 + X1; tier 2 works today |
| Keyword filtering / blocked words | **Yes, on phone** | `RuleEngine`, before storage |
| Priority words | **Yes** | Mirrors `NotificationProcessor.kt:108` |
| Priority contacts | **Yes, degraded** | Free-text match against `sender`; and `sender` is often absent on tier 1 |
| Priority apps | **Yes** | Keyed on the synthesised `packageName` (§6.1) |
| Study Mode | **Yes, with lag** | Enforced on phone from the cached snapshot; a desktop toggle applies on the *next* delivery (§10.3) |
| Masked previews | **Yes** | `contentHidden`, set on phone |
| Notification history | **Yes** | Desktop store unchanged; phone keeps a bounded local copy |
| Per-app mute/allow | **Yes** | via `RULES_UPDATE`, same lag as Study Mode |
| App inventory + icons | **No** | iOS cannot enumerate installed apps and this route carries no icon. Desktop must render an "apps seen" list for iOS peers, with initials, and say why |
| Notification actions / reply | **No** | No channel exists on any route we can ship |
| Dismissal sync phone→desktop | **No** | The trigger fires on arrival only |
| Pairing | **Yes** | Same QR, same compact v3 |
| Reconnect | **Redefined** | §10.1 |
| Encrypted transport | **Yes** | Same Noise crate, same AES-GCM envelope |
| LAN + relay | **Yes** | Same endpoints |

Every "No" and "degraded" row must be visible in the desktop UI for an iOS peer via
capability negotiation — not merely documented here.

---

## 10. Desktop-side changes required

All are additive and behind a peer-capability check; none change Android behaviour.

### 10.1 Connection state for an episodic peer

Today "connected" means a live socket, probed every 3 s with a 6 s timeout
(`server/heartbeat.rs`). An iOS peer connects for a few seconds per notification. Without
change the indicator will flap constantly and read as broken.

Add a peer *kind*. For `kind == episodic`:
- Show **Active** while `now - lastAcknowledgedEvent < staleAfter` (default 30 min),
  **Idle** beyond that, **Not capturing** when a capture-health report says a source
  stopped, and **Offline** only when authentication itself fails.
- Keep the four independent signals from report 19 §7: transport reachable, peer
  authenticated, capture permission present, last acknowledged event.

### 10.2 The connection rules still bind — and this is where they will break

Per `docs/behaviour-checklist.md` and the stated rules: the phone's reconnect toggle is
absolute, and **disconnect means disconnect**. An episodic peer re-attaches on every
notification, which makes rule 2 *more* important, not less.

The rule from the memory applies verbatim: **the attach decision must not ask which
transport the phone arrived on.** After a manual disconnect, an iOS peer's `AUTH` must be
refused at authentication, next to the setting — on LAN and relay alike — and the phone
must then stop attempting until the user picks it under previous connections or scans a
fresh code. Because disconnecting retires the on-screen code, an iOS peer holding the old
pairing key must be rejected exactly as an Android one is.

Add checklist rows for both before any commit.

**And here is a concrete bug this will hit, found by reading the code.**
`may_attach` (`ws_server.rs:640-650`) is correctly transport-agnostic — the fix the
memory records is in place. But its fourth argument is a *one-shot allowance*, and the
comment says so: *"The allowance is taken only when it is needed, because taking it
consumes it."*

An episodic iOS peer opens a new connection **per notification**. So with automatic
reconnection **off**, an iOS phone would be admitted once, consume the allowance, and be
refused on every notification after that. It would look exactly like a broken capture,
and it would be blamed on the intent.

This is a rule conflict, not an implementation detail, so per the project's own practice
it must be settled explicitly rather than patched in code:

> **Design question for the owner.** For an Android phone, "automatic reconnection off"
> means *ask me every time a PC reconnects*. For an iPhone there is no persistent
> connection to reconnect — there is one TCP episode per notification. Asking every time
> would mean a prompt per notification, which is absurd.
>
> The proposed reading, which preserves both stated rules: an episodic source is
> **admitted once**, and that admission persists across episodes until it is revoked.
> Disconnect revokes it (rule 2 holds — the phone stops being accepted, on LAN and relay
> alike). The phone's reconnect toggle still governs whether that first admission needs
> an explicit approval (rule 1 holds). What changes is only that the allowance is not
> re-consumed per episode.
>
> Do not implement this until it is agreed. If it is agreed, it needs its own
> `behaviour-checklist.md` rows and its own tests alongside `may_attach`'s existing ones
> at `ws_server.rs:683-735`, because this is precisely the area where fixes on this
> project have repeatedly undone one another.

Related, and cheaper: `send_notification_ack` (`ws_server.rs:652-680`) sends one ACK per
notification. A 40-item batch therefore costs 40 desktop→phone frames against the
desktop's own relay token budget (§7.5). Either cap batches accordingly or add a batched
ACK — but a batched ACK is a protocol change and needs Android compatibility thought
through, so the cap is the v1 answer.

### 10.3 Rules delivery

The phone filters from a cached snapshot, so a desktop rule change lands on the *next*
delivery. Add `version` to `RULES_UPDATE` (the field exists in `RulesUpdatePayload`
already) and have the desktop display "rules v*N* applied on phone" from `RULES_ACK`.
For Study Mode specifically, show the phone's last-applied state rather than the
desktop's intended state.

### 10.4 Optional: waking the phone

The existing `DESKTOP_ACTION{action:"reconnect_request"}` needs a live socket, which an
iOS peer usually lacks. A silent APNs push could nudge it. This is feasible on the
current stack — Cloudflare Workers have WebCrypto ECDSA P-256, which is what APNs
token auth (ES256 JWT) requires, and it needs no new hosting.

**But treat it as v2 and be honest about the ceiling:** silent pushes are best-effort,
Apple's own guidance is roughly two to three background notifications per hour, Low Power
Mode blocks them, and a force-quit app receives none. It can improve Study-Mode latency;
it cannot deliver a guarantee.

### 10.5 Typed notification contract

Before adding a third client, fix what report 19 §7 item 4 identified: the desktop
currently plucks fields from an untyped `serde_json::Value` with silent defaults
(`store.rs:481-501`) — a missing `appName` becomes `"Unknown"`, a missing `id` is
invented. Define the record once in `desktop/core`, `Deserialize` it, and reject
malformed input. Do this **before** iOS starts sending, or the first field-name typo will
be silently absorbed and cost a day.

---

## 11. Testing

**Cross-language golden vectors** — the highest-value test asset, because three clients
must agree. Add fixtures under `shared/testdata/` and consume them from Rust, Kotlin and
Swift:
- compact-v3 QR bytes → expected `QrPayload` (extends the existing `CompactPairingPayloadTest`).
- pairing key → derived AES-GCM key, and a known nonce+plaintext → ciphertext.
- a Noise transcript with fixed keys → expected handshake hash and first transport message.
- record framing: a 150 KiB record → the exact chunk headers.

**iOS unit tests:** rule evaluation parity with the Kotlin cases, dedup window, outbox
state machine, envelope encode/decode against the goldens, compact-QR decode.

**Device tests (Simulator cannot cover these):** local-network prompt and denial path;
intent execution locked, after reboot-without-unlock, in Low Power Mode, in a Focus;
burst of 50 notifications in 10 s (expect batching, expect no 4008); LAN→relay failover
by disabling Wi-Fi mid-episode; permission revoked mid-run; automation deleted (capture
health must go red); force-quit then a new notification.

**Desktop regression:** the whole `behaviour-checklist.md`, plus new rows for episodic
connection state and for disconnect-refusal of an iOS peer on both transports.

**CI:** add an `ios-ci` workflow on `macos-latest` — build the Rust XCFramework, then
`xcodebuild test` on a simulator for the unit tests. And, per report 19 B7, turn on
`cargo test` for `desktop/src-tauri` and `shared/secure-channel` on all three OSes; today
macOS is `cargo check` only and the Noise crate is never tested at all. **Do that first** —
adding a client to an ungated suite is how the existing fixes keep undoing each other.

---

## 12. If X1 fails — and per §1.1, expect it to

Do not build §3. Revised fallbacks, in the order the evidence now supports:

1. **Desktop-side ANCS becomes the primary route** (report 19 §4). This is no longer the
   consolation prize. It is the only route with *proven* access to other apps'
   notification content on a stock iPhone: reproduced in 2026 against iOS 26.5 and an
   iOS 27 beta by two independently maintained open-source projects. It gives full
   title/subtitle/body, the app's bundle id and display name, twelve categories,
   add/**modify**/**remove** events, and two actions — with **one** consent and **no**
   per-app automations and **no iOS app to install at all**. What it costs: Bluetooth
   range at the capture hop, no icons, no arbitrary reply, and content that degrades
   with the user's Show Previews setting.

   Its open question is Windows (experiment X2), and its answer on macOS is a
   documented no. Linux is proven today. So the honest v1 shape becomes
   **iPhone → Windows PC, nearby**, gated on X2 — with X5 (reproduce tincan or blueferry
   on Linux) as the cheap control that tells you whether an X2 failure is Windows' fault
   or yours.

2. **Tier 2, only if X6 says it carries content.** Per §1.2 this is no longer a safe
   assumption. If the communication triggers turn out to be filter-only as well, this
   tier does not exist.

3. **Filter-as-encoding.** Because a filter matches on Message/Title/Subtitle, N
   automations each invoking the intent with a *different hardcoded* `@Parameter` can
   transmit a low-bandwidth enumerated signal — "a notification matching category K
   arrived from app X" — without ever reading content. This is a real, bounded capability
   that survives even the worst X1 outcome. It supports a genuine product ("tell my PC
   when something urgent arrives from Slack") but it is **not** notification mirroring
   and must never be described as such. Automation count limits are undocumented, so
   probe them before designing around it.

4. **Own-app notifications.** `UNNotificationServiceExtension` and
   `getDeliveredNotifications` give full content — but only for FocusBridge's own
   notifications. Useful for a provider-integration product (report 19 §5 route D), not
   for mirroring.

5. **Manual share only.** Honest, small, not worth an App Store listing alone.

**Closed, do not revisit:** reading Notification Center from a shortcut (no such action
exists; `getDeliveredNotifications` is scoped to your own app), and screenshot+OCR (a
shortcut cannot capture the screen programmatically, and nothing can capture a locked
screen).

---

## 13. Risks and rollback

| Risk | Mitigation |
|---|---|
| X1 fails | §12. Cost so far is one afternoon, no code |
| iOS 27 changes the trigger in a point release | Tier 2 keeps working; capture health surfaces the break immediately |
| App Review objects to the pattern | No prohibition found (report 19 §2.5), but that is a negative search result. De-risk via TestFlight before a public listing |
| ATS / self-signed rejection | Network.framework path avoids ATS entirely; that is why it was chosen |
| Relay 4008 storms | §7.5 client rate limiting, plus a desktop-visible rate-limited state |
| Rust XCFramework build friction in CI | Keep the build script standalone and runnable locally; commit the cbindgen header so a stale generator cannot silently change the ABI |
| Scope creep into actions/icons | §9 is the contract; anything not in it needs a new report |

**Rollback scope for this document:** deletion of this file and its report-15 index entry.
No application code, dependency, migration, credential or deployed service is changed by
this blueprint. The iOS app is a new top-level `ios/` directory and a new `rust/secure-channel-ffi`
crate; neither touches the Android app, the desktop app, the relay, or `shared/secure-channel`.

---

## 14. Order of work

0. CI gate: `cargo test` on desktop + secure-channel, all three OSes. (Report 19 B7.)
1. **Run X1.** Half a day, no code. Decide.
2. Desktop: typed notification contract (§10.5) + episodic peer state (§10.1/10.2) + checklist rows.
3. `rust/secure-channel-ffi` + XCFramework + golden vectors. Verifiable with no iPhone.
4. iOS skeleton: pairing (QR + compact v3), Keychain, store, local-network onboarding.
5. Relay transport, then LAN transport. Prove one delivery end-to-end from the app in the foreground.
6. `ImportNotificationIntent` + outbox + batching + rate limiting. Prove delivery from a locked phone.
7. Tier 2 intents, automation setup guide, capture health.
8. Device test matrix (§11), then TestFlight.

Steps 0–3 are unblocked today and do not depend on X1. Step 1 decides everything after it.

---

## Appendix A — `RuleEngine` parity specification

The Swift rule engine must reproduce the Android pipeline exactly, or the same
notification will be filtered differently on the two phones and the desktop's rules UI
will mean two different things. Read from `NotificationProcessor.kt`,
`PriorityEngine.kt`, `UrgencyDetector.kt` and `NotificationFilter.kt` at `cdf418f`.

### A.1 Order of operations (do not reorder)

1. **Source filter.** Android drops ongoing, foreground-service and group-summary
   notifications, drops `android` / `com.android.systemui`, and requires a non-blank
   title or text. iOS equivalent: drop records whose source resolves to FocusBridge or
   Shortcuts (§7.8), and drop records where title *and* body are both blank.
2. **Per-app mute.** `appRule.muted == true` → drop.
3. **`searchableText`** = `[sender, message, appName, packageName]`, dropping nils,
   joined with a single space, then **lowercased**. Note it deliberately includes the app
   name and package — this is why a priority contact "sam" also matches
   `com.samsung.*`. Reproduce the behaviour; do not silently "fix" it, or the two
   platforms diverge. Raise it as a product question instead.
4. **Blocked keywords.** If any blocked keyword is a substring of `searchableText` → drop.
5. **Base classification** (`PriorityEngine.classify`, on the **message only**):
   - two-factor code → `URGENT`
   - urgent → `HIGH`
   - `packageName` contains `instagram` or `tiktok` → `LOW`
   - otherwise `NORMAL`
6. **Override** (`overridePriority`), in this order:
   - if already `URGENT` → keep it, no further checks
   - else if the app rule has `priority == true` → `HIGH`
   - else if a favourite contact matches (`sender` contains it, **or** `searchableText`
     contains it) or a priority keyword is a substring of `searchableText` → `HIGH`
   - else keep the base classification
7. **Study Mode.** If study mode is on **and** the app is not `studySafe` **and**
   priority < `HIGH` → drop. (`Priority` ordinal order: LOW < NORMAL < HIGH < URGENT.)
8. **Masking.** `contentHidden = parsed.contentHidden || privacyMode`.
9. **Identity.** `id = contentStableNotificationId(...)`, `batchId = baseId` (§A.3).

### A.2 `UrgencyDetector`, verbatim

```
urgentWords = ["urgent", "emergency", "asap", "important", "call me"]
codeWords   = ["code", "verify", "verification", "otp", "authentication", "pin"]

isTwoFactorCode(m):  t = lowercase(m ?? "")
                     return any(w in t for w in codeWords)
                            and regex(/\d{4,8}/).matches(t)

isUrgent(m):         t = lowercase(m ?? "")
                     return any(w in t for w in urgentWords) or isTwoFactorCode(t)
```

Two things to carry over deliberately: matching is plain **substring** containment, not
word-boundary; and `\d{4,8}` in Swift `NSRegularExpression` behaves the same, but
verify against the shared vectors rather than assuming.

### A.3 Identifier derivation

```
canonical = [ packageName,
              lowercase(trim(sender ?? "")),
              lowercase(trim(message ?? "")),
              String(timestamp),
              String(index) ].joined("|")
id        = "content:" + hex(SHA256(utf8(canonical))).prefix(24)
```

`index` is the position within one source notification's expansion. Android also carries
`batchId = baseId` where `baseId` is the platform notification key — iOS has no such key,
so `batchId` must be **omitted**, matching §6.1. Note the desktop discards `batchId`
anyway (`db/models.rs` has no such field), so nothing downstream depends on it.

Per §7.4, iOS adds a monotonic per-device sequence to the hash input so two identical
messages at the same millisecond stay distinct. **That is a deliberate divergence** —
record it in the checklist, and keep the rest byte-identical so the shared vectors still
pin the common path.

### A.4 Rules configuration

`RulesUpdatePayload` (`Protocol.kt:73-80`) carries `version`, `appRules`
(`{packageName, muted, priority, studySafe}`), `priorityKeywords`, `blockedKeywords`,
`favoriteContacts`. Android stores keyword lists as CSV and normalises on load: split on
`,` and newline, trim, lowercase, drop blanks. **Swift must apply the identical
normalisation**, or a rule typed with a capital letter or a trailing space will work on
one phone and not the other.

`privacyMode` and `studyModeEnabled` are local config keys on Android
(`privacy_mode_enabled`, `study_mode_enabled`, string `"true"`). On iOS they arrive in
the snapshot; persist them with the snapshot's `version` so `RULES_ACK` can report
exactly which version the phone is enforcing (§10.3).

---

## Appendix B — pre-coding checklist

Tick every line before writing Swift. Each maps to a section above.

- [ ] X1 run and recorded, with the field list it produced (§1)
- [ ] `cargo test` gated in CI for `desktop/src-tauri` and `shared/secure-channel` (§14.0)
- [ ] Typed notification contract landed on the desktop, malformed input rejected (§10.5)
- [ ] Episodic peer state + disconnect-refusal on **both** transports, with checklist rows (§10.1, §10.2)
- [ ] Golden vectors committed and passing from Rust and Kotlin before Swift exists (§11)
- [ ] XCFramework builds locally and in CI; cbindgen header committed (§4, §13)
- [ ] Deployment target, bundle id, App Group and Keychain access group decided
- [ ] `NSLocalNetworkUsageDescription` written, and foreground permission flow designed (§7.3)
- [ ] Decision recorded: `supportedModes = .background`, no dialog, no disambiguation (§7.1)
- [ ] Keychain `AfterFirstUnlockThisDeviceOnly` + file protection `CompleteUntilFirstUserAuthentication` (§7.2)
- [ ] Client rate limiter ≤1 frame/600 ms and batch cap ~40 (§7.5)
- [ ] Self-notification guard against feedback loops (§7.8)
- [ ] Capability negotiation shipped on the desktop so absent features render as unavailable (§9)

---

## Appendix C — step 0, exactly

Step 0 is the CI gate, and it is the one item here that is unambiguous, unblocked, and
independent of every Apple question. Do it first.

**What is true today** (`.github/workflows/desktop-ci.yml`, verified at `cdf418f`): the
matrix is `ubuntu-22.04 / macos-latest / windows-latest`, and the final Rust step is

```yaml
      - run: cargo check --locked
        working-directory: desktop
```

`cargo check` does not build test targets, so **no desktop Rust test has ever run in
CI on any OS** — not the three integration files in `desktop/src-tauri/tests/`, and not
`ws_server::attach_tests`, `pairing_cmd::pairing_request_tests`, `relay_client::tests`,
`state::vault_lock_tests` or `tls_provider_tests`. Those are exactly the tests
`behaviour-checklist.md` names as the guarantee for the rules that keep regressing.

Separately, `shared/secure-channel` and `shared/secure-channel-jni` appear in **no
workflow at all**, so the Noise implementation's own `tests/sessions.rs` and
`tests/public_vector.rs` never run either.

**Minimum change:**

```yaml
      - run: cargo test --locked
        working-directory: desktop
```

replacing the `cargo check` line (test implies check), plus a new job:

```yaml
  shared:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-22.04, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v6
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --locked
        working-directory: shared/secure-channel
      - run: cargo test --locked
        working-directory: shared/secure-channel-jni
```

Expect this to be *slower*, and expect it to surface failures — particularly on macOS,
which has only ever been compile-checked, and where `security/database_key.rs:117`'s
Keychain test has never executed. **Those failures are the point.** Do not add the iOS
client on top of a suite nothing runs; per the project's own record, that is the
mechanism by which fixes here undo one another.

Also worth adding while in the file: `relay-worker/` has 65 vitest tests and is the
*live* relay, yet has no workflow. `relay/` — the unused Actix service — has the most
rigorous one. That inversion should be corrected in the same pass.

---

## Appendix D — App Intents traps, from developers who hit them

These are failure reports, not theory. Each has cost someone days.

**D.1 — Intents silently absent because metadata export failed.** App Intents code can
compile green while no intents are exported at all; the only signal is a *warning* in the
build log: `At least one halting error produced during export. No AppIntents metadata have
been exported and this target is not usable with AppIntents until errors are resolved.`
One real cause was a non-exhaustive `caseDisplayRepresentations`. **Make grepping the
build log for `appintentsmetadataprocessor` a pre-commit check.**
— https://www.adamrussell.com/appintent-not-showing-in-shortcuts-app

**D.2 — Shortcuts caches the intent catalog.** First discovery has required a **full OS
restart**; clean build, Xcode restart and Shortcuts restart were all insufficient
(forums/thread/768630). A separate report says renaming an existing intent does not
propagate at all — FB15638502, no Apple response (forums/thread/770273). Budget for this
in the dev loop or you will spend hours debugging working code.

**D.3 — `AppShortcutsProvider` inside an extension is unreachable from the app.** No
solution given by DTS; shared frameworks and dual target membership did not fix it
(forums/thread/764792). **This is a further argument for hosting the intent in the app,
as §4 already chose.** Decide where the provider lives before splitting targets.

**D.4 — Provider syntax quirks that make intents vanish.** Phrases must interpolate the
app name — `phrases: ["\(.applicationName) Import Notification"]`, not
`["Import Notification"]` — and the provider must use builder syntax, not a returned
array (forums/thread/711166).

**D.5 — "Invalid action metadata" after roughly an hour backgrounded.** The most alarming
one for us, because our app is backgrounded essentially always. One intent of nine failed,
only after 1+ hour in the background; running a different working intent temporarily
un-stuck it. The developer swapped `perform()` bodies between two intents and **the
failure followed the intent identity, not the code**. The one structural difference: the
failing intent *wrote* to a database; the working one only read. Unresolved, no Apple
response (forums/thread/784571). **Our ingest intent writes to a database. Treat this as a
named risk, reproduce it deliberately in the device test matrix, and have a fallback
(a second registered intent identity to fail over to) sketched before shipping.**

**D.6 — Locked execution is more reliable in documentation than in practice.** The policy
is permissive (`.alwaysAllowed`), but there are unresolved reports of automations stuck on
"device needs to be unlocked" with every relevant setting enabled
(discussions.apple.com/thread/255538383), and of `openAppWhenRun = true` on a locked phone
interrupting the shortcut so the result never returns (forums/thread/732466).
**Rule: `supportedModes = [.background]`, default `authenticationPolicy`, never a
foreground mode on the ingest path.**

**D.7 — `LongRunningIntent` forces a visible Live Activity.** The system renders progress
automatically from `performBackgroundTask`. A Live Activity per notification is a UX
dealbreaker, so **stay under the 30-second budget and do not adopt it** for ingest. A
single POST is far inside that budget. (macOS has no 30-second limit; iOS and the rest do.)

**D.8 — Automations are flaky by design, and each release retunes them.** "Run
Immediately" must be set or a missed banner is a lost run; Low Power Mode pauses or delays
automation background activity; iOS 18 narrowed which actions run without user presence
and iOS 26 tightened scheduling for low-priority automations. A current beta report says
"Shortcuts are absolutely broken in beta 2, especially the automations", with a pattern of
an automation working once after editing and then failing again.

**Consequence for the desktop side:** it must tolerate dropped, delayed and out-of-order
events, and must never equate one notification with one delivered message. That is already
the design in §7.4 and §10.1 — D.8 is why it is not optional.
