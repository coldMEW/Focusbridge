# Engineering the iPhone app

Assumes [`01-iphone-as-source.md`](01-iphone-as-source.md) has been read and that
R-01 (can a Mac be an `ASAccessory`?) has come back positive. If it has not, most
of this document is moot — read §7 only, which covers the iPhone as a *receiver*.

---

## 1. The stack

**Recommendation: native Swift + SwiftUI, with a Rust core delivered as an
xcframework. Not Tauri.**

This is not a close call, and the reason has nothing to do with taste:

**The product is three app extensions.** `AccessoryDataProvider`,
`AccessoryTransportSecurity` and `AccessoryTransportAppExtension` are Swift
extension targets with specific `EXExtensionPointIdentifier` values, specific
entitlements, and protocol conformances declared in Swift result-builder syntax
**[VERIFIED]**. There is no webview in an app extension and no Tauri abstraction
over these extension points. The extensions are the app; the UI is the small part.

Supporting reasons:

- Tauri's iOS support is functional but younger than its desktop support, and
  weaker for apps that lean on native platform capabilities **[REPORTED]**. This
  app is nothing but native platform capabilities.
- `AccessorySetupKit`, `AccessoryNotifications`, `CoreBluetooth`, Keychain and
  `Network.framework` are all Swift-first.
- Extensions have tight memory ceilings. A webview runtime inside one is a
  non-starter.

Rejected: Tauri iOS (above), Kotlin Multiplatform (no benefit — the shared logic
is Rust, not Kotlin), Flutter / React Native (same extension problem, plus a second
FFI layer).

**Consequence to accept up front:** unlike the desktop, the phone apps do *not*
share a UI codebase. Android is Compose, iOS is SwiftUI. That is already true of
Android versus the Tauri desktop, so it is not a new kind of cost — but it does
mean the iOS app is a genuine second client, not a port.

---

## 2. Reusing the Rust core

The Noise engine must not be reimplemented. `5bd8256` established the rule and the
reason: "Two implementations of the same protocol are two implementations that can
disagree, and a disagreement in a handshake is a security bug." Android reaches it
through JNI; iOS reaches it through an xcframework.

### Targets

```
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
```

- `aarch64-apple-ios` — devices
- `aarch64-apple-ios-sim` — simulator on Apple silicon
- `x86_64-apple-ios` — simulator on Intel (drop if no Intel Mac is in the loop)

Build a static library per target, then:

```
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libfocusbridge_secure_channel.a \
    -headers include/ \
  -library target/aarch64-apple-ios-sim/release/libfocusbridge_secure_channel.a \
    -headers include/ \
  -output FocusBridgeSecureChannel.xcframework
```

Note the simulator slices must be a **separate library entry**, not `lipo`-ed
together with the device slice — an xcframework separates by platform *and*
environment, and merging device with simulator produces an archive Xcode rejects.

### Binding layer

| Option | Verdict |
|---|---|
| **`cbindgen` + a hand-written C header, consumed through a module map** | **Chosen.** The API surface is tiny and handle-based: create session, write message, read message, destroy. The existing `secure-channel-jni` crate already proves that shape. A C header is the least fragile thing that can work, has no build-time codegen to break, and is trivially auditable — which matters for a crypto boundary. |
| UniFFI | Rejected for this crate. Designed for rich cross-language types; here it would add a codegen step and a runtime for four functions. Reconsider if the Swift↔Rust surface ever grows to include the protocol types. |
| swift-bridge | Rejected — same reasoning, less maturity than UniFFI. |
| `cargo-swift` / `cargo-xcframework` | Useful convenience wrappers; evaluate for the build script, but do not let them own the API shape. |

### Repository layout

Mirror what Android already does:

```
shared/
├── secure-channel/          the one implementation (unchanged)
├── secure-channel-jni/      Android wrapper  (exists)
└── secure-channel-ffi/      NEW: C ABI wrapper for Apple
```

`secure-channel-ffi` reuses the **same handle-registry design** as the JNI wrapper:
opaque process-local ids, capped, never reused, retired on any error. Do not invent
a second lifetime model — that registry is a security property (`5bd8256`), not an
implementation detail. Factor the registry into the shared crate if it can be done
without disturbing Android; otherwise duplicate it deliberately and test both.

### Commit the artefact, or build it in CI?

Android commits the four `.so` files, on purpose: "so a CI-built APK is not
silently broken."

**Recommendation: build the xcframework in CI, do not commit it.** The reasoning
that justified committing on Android — a CI-built APK missing an ABI fails only at
the first cross-network connection — is answered better here by a build-time check.
An iOS build that cannot link the static library **fails at link time**, loudly,
which is exactly the failure mode Android could not get. Add a CI assertion that
the xcframework contains every expected slice, and keep the repository free of a
multi-megabyte binary that must be regenerated on every Rust change.

---

## 3. TLS, pinning, and where to put the socket

The Android client pins the desktop's self-signed certificate by DER SHA-256 from
the QR. iOS has three ways to do the same, and the choice interacts with App
Transport Security.

| Design | How | Verdict |
|---|---|---|
| **TLS inside Rust (rustls) over a plain TCP socket** | Swift opens a raw `NWConnection` in TCP mode; Rust does the handshake and pinning | **Recommended.** Reuses the exact verification code the desktop already trusts, sidesteps ATS entirely because the system never sees a TLS session it must judge, and keeps one pinning implementation across all platforms. |
| `Network.framework` + `sec_protocol_options_set_verify_block` | Custom trust evaluation in Swift | Viable, and the idiomatic Apple answer. Costs a second pinning implementation to keep in step with the Rust one. |
| `URLSessionWebSocketTask` + `urlSession(_:didReceive:)` | Override trust in the delegate | Weakest option. Least control, and the WebSocket task's behaviour under backgrounding is the least predictable. |

**App Transport Security**: connecting `wss://` to a private IP address with a
self-signed certificate is precisely what ATS is designed to refuse.
`NSAllowsLocalNetworking` is the intended exception for local-network endpoints and
does not require the blanket `NSAllowsArbitraryLoads`, which does attract review
questions. **[UNVERIFIED — whether ATS applies at all when TLS is performed inside
Rust over a raw socket. It should not, since ATS governs `URLSession` and
`Network.framework` TLS, but this must be tested, not assumed. Risk R-11.]**

**Local network permission** (`NSLocalNetworkUsageDescription`, iOS 14+): connecting
to a private IP triggers it. Same rules as the Mac side — provide a real usage
string, detect denial, and make sure the relay path still works when it is denied.

---

## 4. Background execution — and why it matters less than you think

For a conventional design this is where iOS projects die. An iOS app cannot hold a
persistent WebSocket in the background: `beginBackgroundTask` buys roughly thirty
seconds, `BGAppRefreshTask` and `BGProcessingTask` are opportunistic with
latencies measured in tens of minutes to hours, silent push is throttled, and the
`voip` background mode is no longer available for this kind of use.

**The AccessoryNotifications design does not need any of that.**

The system invokes `AccessoryDataProvider.addNotification(_:alertingContext:)`
when a notification occurs **[VERIFIED]**. The extension is woken by the OS, per
notification, as a first-class part of the notification pipeline. There is no
socket to keep alive and no foreground service to fight for. That is why Apple's
model is *better suited to this product than Android's*, where the app must run a
foreground service permanently to hold a connection open.

What remains uncertain, and is risk R-12:

- **How long does the transport extension get?** It must move bytes to the
  accessory. Over Bluetooth to a nearby device this is milliseconds. Over
  `.internet` it means a TLS connection to the relay, a Noise handshake if none is
  cached, and a write. Whether the extension's lifetime and network entitlement
  accommodate that is **[UNVERIFIED]** and is the second-most-important thing to
  test after R-01.
- **Mitigation if it does not:** keep a warm relay session in the containing app
  when it is running, and have the extension hand off through a shared App Group
  container plus a `BGProcessingTask` for the tail. Degrade honestly — tell the
  user "delivered when your Mac is nearby" rather than pretending.

### If a persistent connection is ever needed anyway

`bluetooth-central` and `bluetooth-peripheral` background modes do allow long-lived
BLE work, with state restoration. Background BLE advertising is heavily degraded —
the local name is dropped and service UUIDs move to the overflow area, which makes
a backgrounded iPhone hard to discover. Since the iPhone is the *central* in the
ASK model and the Mac is the advertising peripheral, this asymmetry works in our
favour. **[UNVERIFIED — none of this has been measured.]**

---

## 5. Storage and secrets

| Concern | Choice |
|---|---|
| Local history | SQLCipher, via `rusqlite` compiled into the Rust core, so one storage implementation serves iOS and the desktop |
| Database key | Keychain, `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` |
| Why "after first unlock" | The extensions run when notifications arrive, including while the screen is locked. `WhenUnlocked` would make the key unreadable exactly when it is needed. `ThisDeviceOnly` prevents it syncing to iCloud. |
| Sharing with extensions | A **Keychain access group** plus an **App Group** container, so the data provider, the transport extension and the app all reach the same rules snapshot and the same session state |
| Noise static key and PSK | Keychain, same accessibility class |

The Android app holds its key in the Android Keystore and the desktop in DPAPI or
(on Mac) the Keychain. The pattern is consistent: the OS holds the key, the user
never types it, and background work survives a locked UI. Keep that invariant.

**[UNVERIFIED]** Whether SQLCipher's vendored OpenSSL builds cleanly for
`aarch64-apple-ios` is unconfirmed. Fallback: the SQLCipher community build via
SPM, accepting a second SQLCipher in the process, or per-field encryption using
the Noise crate's primitives for the small amount of data the phone actually keeps.
Prefer the first; the phone's local history is a convenience, not the product.

---

## 6. Scanning the pairing QR — a real, specific risk

The desktop emits a **compact binary v3 payload**, about 407 characters. On Android
this is decoded from raw bytes; `CompactPairingPayloadTest.readsEveryFieldTheDesktopWrote`
decodes bytes the Rust encoder actually produced.

iOS QR APIs — `AVCaptureMetadataOutput` (`AVMetadataMachineReadableCodeObject`),
Vision's `VNDetectBarcodesRequest`, and `DataScannerViewController` — surface a
decoded **string**. A QR in byte mode carrying non-UTF-8 bytes can be mangled or
dropped by a string-oriented API. This project has already been bitten once by
assuming a QR would round-trip (`3d960f1`, `fab5b75`).

**[UNVERIFIED — and it must not be assumed either way.]** Risk R-13. Two safe
paths, decided by experiment:

1. If a raw-bytes descriptor is reachable (Vision's barcode observation exposes
   payload data in addition to the string on recent OS versions), use it and keep
   the v3 payload unchanged.
2. If not, the v3 payload must be constrained to a character set every scanner
   round-trips losslessly — the existing JSON fallback already is, and a
   base45/base64url-encoded v3 would be too, at a modest size cost. The QR budget
   is 425 characters with the current symbol size; measure before choosing.

Do **not** ship until a code produced by the Rust encoder has been decoded on a
real iPhone and compared byte for byte, exactly as the Android test does.

Note that in the AccessoryNotifications design the QR may not be needed at all —
AccessorySetupKit's picker replaces it. Keep the QR path for the *receiver* mode
and for LAN-only pairings.

---

## 7. The other iPhone product: iPhone as receiver

Distinct from everything above, and worth naming because it is not region-locked.

An iPhone showing notifications *from* a Mac or Windows PC is straightforward and
useful — but the delivery mechanism must be **APNs**, because nothing else wakes an
iOS app reliably. That has a consequence the project must not paper over:

**APNs payloads pass through Apple's servers.** The privacy promise says
"notification content is readable by exactly two machines". A naive push would
break it.

The design that preserves it:

1. The desktop encrypts the notification with the existing Noise session.
2. The ciphertext is pushed as an APNs payload with `mutable-content: 1`.
3. A **`UNNotificationServiceExtension`** on the phone — which exists precisely to
   "decrypt an encrypted data block" before display, in Apple's own words
   **[VERIFIED]** — decrypts it using a key from the shared Keychain access group
   and rewrites the notification content.
4. Apple sees ciphertext. The promise holds.

Constraints: the APNs payload limit is 4 KB, so long messages and attachments need
a fetch-on-decrypt rather than inline delivery. Pushing requires an APNs key from
the Apple Developer Program, and a server component to send it — the Cloudflare
Worker can do this, but it becomes a *sender* rather than a pure router, which is
a meaningful change to the relay's threat model and must be written down before it
is built.

**Recommendation: do not build this in the same phase as the source path.** It is a
different product with a different architecture, and mixing them will muddle both.

---

## 8. Review and compliance

- **Encryption export compliance.** The app ships a custom Noise implementation.
  `ITSAppUsesNonExemptEncryption` must be declared truthfully in `Info.plist`, and
  an annual self-classification report to US authorities is generally required for
  apps using non-exempt encryption. **[UNVERIFIED — confirm the current
  obligations; they change.]** This is paperwork, not a blocker, but it is
  paperwork that must not be discovered at submission time.
- **Companion-app guidelines.** Apple requires apps not to be useless without
  their accessory. The FocusBridge iOS app has an inbox, rules and settings that
  work standalone, so this is satisfiable — but keep it true.
- **Background Bluetooth** use must be justified in the review notes.
- **Sign in with Apple** is required if other third-party sign-in options are
  offered. FocusBridge uses Firebase Auth only to authorise relay provisioning.
  Either add Sign in with Apple, or — better — drop Firebase on Apple platforms
  and verify an Apple identity token directly in the Worker. See
  [`05-shared-core-and-protocol.md`](05-shared-core-and-protocol.md) §6.

---

## 9. Summary table

| Capability | API | Constraint | Confidence |
|---|---|---|---|
| Capture other apps' notifications | `AccessoryNotifications` | iOS 26.5, iPhone, EU customers only | **[VERIFIED]** |
| Pair with the Mac | `AccessorySetupKit` | BLE discovery only for a computer | **[UNVERIFIED]** R-01 |
| Reach the Mac over the internet | `AccessoryTransport.internet` | Documented ordering; extension lifetime unknown | **[UNVERIFIED]** R-02, R-12 |
| Noise session | Rust xcframework | Build pipeline only | High |
| Pinned TLS | rustls inside Rust over raw TCP | ATS interaction untested | **[UNVERIFIED]** R-11 |
| Encrypted local history | SQLCipher + Keychain | iOS build of vendored OpenSSL untested | **[UNVERIFIED]** |
| Scan the v3 QR | AVFoundation / Vision | Binary payload may not round-trip | **[UNVERIFIED]** R-13 |
| Background delivery | System-invoked extensions | Better than Android's model | **[VERIFIED]** for the invocation; lifetime unknown |
| Notification actions | `NotificationResponse` | None known | **[VERIFIED]** |
