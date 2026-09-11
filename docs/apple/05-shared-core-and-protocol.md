# One core, four platforms

How to add macOS and iOS without forking the crypto, the protocol or the rules —
and how to let four clients with genuinely different capabilities talk to each
other without lying about what they can do.

---

## 1. Target repository layout

```
focusbridge/
├── android/                     Kotlin phone app                      (exists)
├── apple/                       NEW
│   ├── FocusBridge.xcodeproj    or a Swift package + xcodeproj
│   ├── iOS/                     companion app (SwiftUI)
│   ├── iOS-DataProvider/        AccessoryDataProvider extension
│   ├── iOS-TransportSecurity/   AccessoryTransportSecurity extension
│   ├── iOS-Transport/           AccessoryTransportAppExtension
│   └── Shared/                  Swift code common to app and extensions
├── desktop/                     Tauri app — gains a macOS build target (exists)
├── shared/
│   ├── secure-channel/          the ONE Noise implementation           (exists)
│   ├── secure-channel-jni/      Android JNI wrapper                    (exists)
│   ├── secure-channel-ffi/      NEW: C ABI wrapper for Apple
│   └── protocol.json            the wire contract                      (exists)
├── relay-worker/                Cloudflare Worker                      (exists)
└── docs/apple/                  this folder
```

The rule that governs this layout: **one implementation of anything security-
relevant, one thin wrapper per platform.** It is already how Android reaches the
Noise engine, and `5bd8256` records why. Apple gets a wrapper, not a port.

### What is shared, and what is not

| Layer | Android | Windows | macOS | iOS |
|---|---|---|---|---|
| Noise engine | `shared/secure-channel` via JNI | direct | direct | via `secure-channel-ffi` |
| Protocol types, QR codec, priority, study mode | Kotlin reimplementation *(existing divergence)* | `focusbridge_core` | `focusbridge_core` | `focusbridge_core` via FFI |
| Storage | Room + SQLCipher | rusqlite + SQLCipher | rusqlite + SQLCipher | rusqlite + SQLCipher |
| UI | Compose | React | React (same) | SwiftUI |

Note the honest wart in row two: Android already has its own Kotlin copy of the
protocol types and QR decoder, held in step with the Rust one by a test that
decodes bytes the Rust encoder produced (`CompactPairingPayloadTest`). **Do not
repeat that on iOS.** Compile `focusbridge_core` into the same xcframework as the
secure channel and expose the codec through the C ABI. One fewer implementation
that can drift, and the iOS extensions need the rules engine anyway.

---

## 2. Building the Rust core for Apple

```bash
# targets
rustup target add aarch64-apple-darwin x86_64-apple-darwin \
                  aarch64-apple-ios aarch64-apple-ios-sim

# per-target static libs
for T in aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-darwin; do
  cargo build --release --target "$T" -p focusbridge-secure-channel-ffi
done

# macOS universal slice (only if Intel Macs are supported)
lipo -create \
  target/aarch64-apple-darwin/release/libfocusbridge_ffi.a \
  target/x86_64-apple-darwin/release/libfocusbridge_ffi.a \
  -output build/macos/libfocusbridge_ffi.a

# one xcframework, device and simulator kept separate
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libfocusbridge_ffi.a     -headers include/ \
  -library target/aarch64-apple-ios-sim/release/libfocusbridge_ffi.a -headers include/ \
  -library build/macos/libfocusbridge_ffi.a                          -headers include/ \
  -output build/FocusBridgeCore.xcframework
```

Pitfalls, each of which has cost other projects a day:

- **Never `lipo` a device slice together with a simulator slice.** They are
  different platforms to Xcode; an xcframework separates them, `lipo` does not.
- **Set a deployment target** on every Rust build (`IPHONEOS_DEPLOYMENT_TARGET`,
  `MACOSX_DEPLOYMENT_TARGET`) matching the Xcode project, or the linker warns and
  App Store validation may object.
- **`-dead_strip` can remove FFI symbols** that nothing in Swift references
  statically. Keep the C header's declarations and, if symbols vanish, mark them
  used rather than disabling dead stripping globally.
- Static libraries are not code-signed; a **dynamic** framework embedded in an app
  must be. Static is simpler here — prefer it.
- Bitcode is dead; do not enable `-fembed-bitcode`.

### Crypto provider

The same Noise profile, the same crate, on every platform — that is the point.
**[UNVERIFIED]** whether the chosen rustls provider and the Noise crate's
dependencies build cleanly for `aarch64-apple-ios`; assembly-heavy crates sometimes
need a feature flag for iOS. First-hour check.

**Do not reach for CryptoKit.** Apple offers Curve25519 and ChaChaPoly, and a
Swift-native Noise implementation is possible. It would be a second implementation
of a handshake, which is the exact thing `5bd8256` forbids, and a handshake
disagreement is a security bug rather than a compatibility annoyance.

---

## 3. Capability negotiation

Four clients with materially different abilities now exist:

| | Android phone | iPhone | Windows PC | Mac |
|---|---|---|---|---|
| Source of notifications | yes | yes (EU, 26.5+) | no | no |
| Notification actions / quick reply | **no** | **yes** | n/a | n/a |
| App inventory with icons | yes | no (no cross-app enumeration) | n/a | n/a |
| Per-notification source icon | via inventory | from the system | n/a | n/a |
| Dismissal sync from the source | custom message | from the system | n/a | n/a |
| Holds a persistent socket | yes (foreground service) | no (system-invoked extensions) | yes | yes |
| Receives over LAN | yes | yes | yes | yes |
| Receives over relay | yes | yes | yes | yes |

Today the protocol assumes both ends can do everything. That has to change, and
the failure mode to avoid is the one this project has already hit repeatedly: a
capability silently doing nothing rather than saying it is absent. `785e368` is
the canonical example — a setting that "appeared to do nothing, because in
ordinary use it did nothing."

**Recommendation: extend the `AUTH` / `AUTH_OK` exchange with an explicit
capability set**, and make the absence of a capability visible in the UI rather
than inferred.

```jsonc
// AUTH payload, additive; older peers omit it and are treated as the v1 baseline
{
  "protocolVersion": 2,
  "platform": "ios",            // android | ios | windows | macos
  "capabilities": [
    "notification.actions",     // can execute an action back on the source
    "notification.dismissSync",
    "notification.sourceIcon",
    "inventory.apps"            // can enumerate installed apps
  ]
}
```

Rules for using it:

1. **Additive only.** An `AUTH` without `capabilities` is the current Android
   behaviour set. Existing Android↔Windows pairs must keep working untouched;
   that is non-negotiable and belongs in the behaviour checklist.
2. **A missing capability is stated, not hidden.** If a Mac is paired to an iPhone,
   the desktop shows reply buttons. If it is paired to an Android phone, it says
   the phone cannot act on notifications — it does not simply omit the buttons and
   leave the user wondering.
3. **Capabilities are advertised, never assumed from `platform`.** The platform
   field is for diagnostics and log lines; behaviour keys off capabilities, so an
   older iOS build does not get treated as a newer one.

---

## 4. The QR payload

Two changes, both driven by findings elsewhere in this folder.

1. **Byte-mode round-tripping on iOS is unproven** (R-13, and
   [`03-ios-app-engineering.md`](03-ios-app-engineering.md) §6). Until an iPhone
   has decoded a code produced by the Rust encoder and matched it byte for byte,
   assume nothing. The v3 format need not change if raw bytes are reachable; if
   they are not, a v4 that is transport-safe text is the fallback, at a size cost
   to be measured against the existing 425-character budget.
2. **A Mac's QR is a Windows PC's QR.** No format change is needed for the macOS
   desktop. The payload already carries LAN candidates, a certificate fingerprint,
   a pairing key and the optional relay and Noise blocks. Nothing in it is
   Windows-specific.

Keep the existing discipline: the compact form falls back to the JSON link for
anything it cannot represent, partial relay blocks leave a pairing LAN-only, and a
truncated code is refused rather than half-read. Those are checklist rows already.

---

## 5. The relay

No change is required to serve a Mac. The Mac is a desktop peer like any other.

For iOS, three things need thought:

1. **Bearer capability delivery.** The Worker authorises a socket by capability in
   the path (`/v1/socket/<64 hex>/<32 hex>/<role>`), which is header-free and
   therefore works with any WebSocket client. Good — no change needed.
2. **Churn.** An iOS client that only exists while an extension is running will
   connect and disconnect far more often than the Android foreground service does.
   The Durable Object hibernates when idle, which is in our favour, but the
   per-role token buckets and session budgets were sized for a long-lived phone.
   **Re-examine those limits before the first iOS build connects**, or the phone
   will be rate-limited for behaving exactly as iOS requires.
3. **No queue, still.** The relay deliberately has none, and Apple's §3.3.3(J)
   rule against cloud storage of forwarded content makes that design *required*
   rather than merely preferred. Do not add a queue to paper over iOS churn.
   Durable retry stays on the endpoints.

If [`03-ios-app-engineering.md`](03-ios-app-engineering.md) §7 (iPhone as
receiver) is ever built, the Worker would need to send APNs pushes, which turns it
from a router into a sender. That is a real change to its threat model and should
be a separate document and a separate security review, not a patch.

---

## 6. Accounts

Firebase Auth exists only to get an ID token the Worker can verify before
provisioning a relay pair. On Apple platforms it is a poor fit:

- App Store guidelines require **Sign in with Apple** wherever other third-party
  sign-in is offered.
- Firebase's web SDK inside a Tauri webview works on the Mac, but on iOS it means
  pulling a large SDK into an app whose only real need is one signed token.

**Recommendation: add Sign in with Apple and verify the Apple identity token
directly in the Worker.** Apple identity tokens are ordinary JWTs signed with keys
published at a well-known endpoint — the Worker already verifies Firebase tokens
the same way, so this is a second verifier, not a new mechanism. Longer term this
may let Firebase be dropped entirely, which removes a dependency the project has
already had to pin advisories out of (`edad4c6`).

---

## 7. CI

Add two jobs, and hold them to a higher standard than `android-ci` currently meets.

| Job | Runner | Does |
|---|---|---|
| `apple-core-ci` | `macos-latest` | Builds the Rust core for all Apple targets, assembles the xcframework, asserts every expected slice is present, runs `cargo test` for `shared/*` |
| `macos-desktop-ci` | `macos-latest` | pnpm install, tsc, vitest, `tauri build --bundles app`, `cargo test`, clippy |
| `ios-ci` (later) | `macos-latest` | `xcodebuild test` against a simulator |

Non-negotiables, learned from the current flaky Android job:

- **Upload test reports and build logs on failure from the first commit.** The
  Android job's failure cannot presently be diagnosed at all because nothing is
  uploaded; do not repeat that.
- **No wall-clock assertions** in any new test. `SyncEngineTest`'s real-time waits
  are the leading suspect for the existing flakiness.
- Pin the Xcode version explicitly rather than tracking whatever the runner image
  ships, so a runner update cannot silently change the build.
