# Risk register and open questions

Every unresolved thing in this folder, with the experiment that settles it. Ordered
by how much of the plan collapses if the answer is bad.

`[UNVERIFIED]` in the other documents always points here.

---

## Product-deciding

### R-01 — Can a Mac (or a Windows PC) be registered as an `ASAccessory`?

**Impact: total.** If no, the iPhone-as-source product does not exist without
shipping hardware, and Phases 4–5 of the roadmap are cancelled.

**What is known.** Notification forwarding is granted to an accessory paired
through AccessorySetupKit **[VERIFIED]**. ASK discovers by Bluetooth (service UUID,
company identifier, manufacturer/service data, name substring, range), by Wi-Fi
SSID, or by Wi-Fi Aware **[VERIFIED]**. Wi-Fi Aware is unavailable on macOS
**[VERIFIED]**, and SSID matching describes an access point. That leaves BLE
advertising, which macOS and Windows can both do.

**What is unknown.** Whether the pairing flow completes for a general-purpose
computer, and whether Apple intends or will accept it. The framework's language
throughout is "an accessory that you develop", which reads as hardware.

**Experiment.** Roadmap Phase 1, Spike A. A macOS `CBPeripheralManager` advertising
a custom service, an iOS app with a matching `ASDiscoveryDescriptor`, and the ASK
picker. Two hours to build, and the answer is unambiguous.

**If negative:** the fallback is a small BLE accessory (an ESP32-class board) that
bridges to the Mac. That is a hardware product and a different project. Say so and
stop, rather than drifting into it.

---

### R-02 — Is `AccessoryTransport.internet` usable for a general relay?

**Impact: high.** Decides whether the iPhone product keeps FocusBridge's
differentiator — working across networks — or becomes another proximity-bound
mirror like Phone Link.

**What is known.** The enum has `bluetooth`, `localNetwork` and `internet`, and
Apple documents the selection order as Bluetooth, then local network, then internet
**[VERIFIED]**. The transport extension is the developer's own code and receives
ciphertext to move **[VERIFIED]**.

**What is unknown.** Whether an extension invoked per-notification gets enough
lifetime and network access to establish or reuse a relay connection; whether
Apple's §3.3.3(J) rule against cloud storage "except where strictly required to
deliver" is read as permitting a routing relay (it should be — the relay stores
nothing — but it is an interpretation).

**Experiment.** Phase 4, step 4. Instrument the transport extension: log invocation
time, available network, and time to first byte delivered over a TLS connection to
the Worker. Compare with the Bluetooth path.

**If negative:** the iPhone product becomes proximity-bound. Still useful, no longer
differentiated. Say so in the marketing rather than hoping.

---

### R-03 — Are the three accessory entitlements freely assignable?

**Impact: high.** `com.apple.developer.accessory-data-provider`,
`com.apple.developer.accessory-transport-security` and
`com.apple.developer.accessory-transport-extension` are documented as required
**[VERIFIED]**, but the documentation does not say whether they are self-serve in
the developer portal or need an Apple request with justification.

**Experiment.** Open the Certificates, Identifiers & Profiles portal on a paid
account and look at the capability list for an App ID. Ten minutes. Worth doing
immediately after paying, before any code.

**If they need a request:** budget weeks, not minutes, and expect to explain the
Mac-as-accessory story — which makes R-01's answer part of the application.

---

## Schedule-shaping

### R-10 — Does macOS TCC intercept the loopback bridge?

**Impact: architectural.** `relay_client.rs` bridges decrypted records into the
local listener over a loopback WSS connection to `127.0.0.1:9173`. That design is
why "storage, ACKs, inventory, rules and diagnostics run on one tested path for
both transports" (`3d121b6`). If macOS's local-network privacy treats loopback as
local network, the relay path breaks on macOS and the bridge must become an
in-process channel — a change to the core of the desktop app.

**Experiment.** Phase 2, step 4, and early. A minimal Rust binary that listens on
127.0.0.1 and connects to itself, run from `/Applications` and from elsewhere, with
and without local-network permission granted.

**Expectation:** loopback should not be treated as local network. Do not rely on
the expectation.

---

### R-11 — Does App Transport Security apply to TLS performed inside Rust?

**Impact: medium.** [`03-ios-app-engineering.md`](03-ios-app-engineering.md) §3
recommends doing TLS in rustls over a raw TCP socket, which should place it outside
ATS's scope. If ATS does apply, an `NSAllowsLocalNetworking` exception is needed and
must survive review.

**Experiment.** A minimal iOS app opening an `NWConnection` in TCP mode to a
self-signed WSS server on a private IP, with rustls doing the handshake, with no
ATS exceptions in `Info.plist`.

---

### R-12 — What lifetime and resources does an `AccessoryDataProvider` extension get?

**Impact: medium-high.** Determines whether rule evaluation, serialisation and
delivery fit inside one invocation.

**Experiment.** Phase 4. Instrument `addNotification` with timing and memory
watermarks; push a burst of notifications and observe termination behaviour.

**Mitigation if tight:** keep the rules snapshot small and pre-compiled in the App
Group container; hand long work to the containing app; never block on a network
round trip inside `addNotification` — build the message, hand it to the transport
extension, return.

---

### R-13 — Does the compact v3 QR payload survive an iPhone scanner?

**Impact: medium.** Blocks iPhone pairing over the QR path, and this project has
already lost time to QR assumptions twice (`3d960f1`, `fab5b75`).

**Experiment.** Phase 1, Spike B. Byte-for-byte comparison against the Rust
encoder's output, on all three iOS scanning APIs.

**If negative:** a v4 transport-safe encoding, sized against the existing
425-character budget, with the v3 and JSON forms retained for compatibility.

---

## Verification chores

Lower impact; each is an hour or less, and each is a thing this folder currently
assumes.

| # | Assumption | How to settle |
|---|---|---|
| R-20 | `rusqlite` `bundled-sqlcipher-vendored-openssl` builds for `aarch64-apple-darwin` | `cargo build` on the Mac |
| R-21 | The same builds for `aarch64-apple-ios` | `cargo build --target aarch64-apple-ios` |
| R-22 | The rustls provider and Noise dependencies build for Apple targets | Same |
| R-23 | Keychain access survives a change of signing identity without prompting per launch | Sign with a development identity, create the item, re-sign with Developer ID, relaunch |
| R-24 | `UNUserNotificationCenter` works from a Tauri Mac app, signed and bundled | First signed build |
| R-25 | Hardened runtime does not require a JIT entitlement for WKWebView | First notarisation attempt |
| R-26 | WKWebView honours the existing CSP and persists `localStorage` | First macOS run |
| R-27 | GitHub Actions macOS runners are free for a public repository | GitHub billing docs |
| R-28 | Apple's development/testing exemption really does let a non-EU device exercise the framework | Spike A, step 4 |
| R-29 | iOS 26.5 is generally released, not still in beta, and its installed base is meaningful | Apple release notes; adoption figures |
| R-30 | Current US encryption export self-classification obligations | Apple's export compliance page |
| R-31 | MDM/supervision genuinely offers no notification-read payload (asserted as a negative in `01`) | Apple Device Management documentation |

---

## Assumptions that would be expensive to be wrong about

Stated separately because they are not experiments, they are judgements.

1. **The EU restriction persists.** The framework exists because of the DMA. If
   Apple were compelled to open it worldwide, the product's reach multiplies; if
   the DMA position shifts the other way, it could narrow. Nothing in the plan
   should assume either. **Do not build anything whose value depends on the
   restriction lifting.**
2. **iPhone Mirroring stays out of the EU.** If Apple ships it there, FocusBridge's
   iPhone product loses most of its reason to exist overnight, since Apple's
   version is free, built in, and needs no accessory. This is the single largest
   *strategic* risk in the folder and no engineering can mitigate it.
3. **iOS 26.5 adoption.** A framework introduced in a point release of a very
   recent OS has a small addressable base for a while.
4. **The developer is not in the EU.** Development and testing work anywhere
   **[VERIFIED]**, but the author cannot be their own daily-driver user. That
   removes the fastest feedback loop this project has had — nearly every fix in
   [`../BLUEPRINT.md`](../BLUEPRINT.md) §6 was found by the author using the app
   on real hardware. Plan for a European tester, or accept slower discovery.

---

## How to use this register

Same discipline as the behaviour checklist: when a risk is settled, replace it with
the answer and the evidence, and add a row to
[`../behaviour-checklist.md`](../behaviour-checklist.md) if it produced a behaviour
worth holding. Do not delete a risk without recording what settled it — the value
of this file is that the next person does not re-run the same experiment.
