# Roadmap

Five phases, each with an exit gate that must be met before the next begins. The
ordering is deliberate: everything cheap and certain happens before anything
expensive and uncertain, and the two decision points that could cancel work come
as early as they possibly can.

The project's release rule applies throughout — *do not commit or push work that
has not met the full production release gate; automated test success alone does
not satisfy it* — as does the behaviour checklist discipline. Every phase below
adds rows to [`../behaviour-checklist.md`](../behaviour-checklist.md).

---

## Phase 0 — Decide, and get a Mac

**Nothing else can start.** Half a day of decisions, plus procurement.

- [ ] Decide whether to pay the $99/year Apple Developer Program fee. The budget
      rule requires this to be explicit ([`06-distribution-and-cost.md`](06-distribution-and-cost.md)).
- [ ] Obtain a Mac. Apple silicon; any current model. Without one there is no
      Apple work of any kind.
- [ ] Decide whether a **European-only iPhone product** is worth building at all.
      This is a product judgement, not a technical one. If the answer is no, stop
      after Phase 2 and the folder still paid for itself.
- [ ] If the iPhone path is live: obtain an iPhone that can run iOS 26.5.

**Gate:** a Mac on the desk and a written answer to the European question.

---

## Phase 1 — The two spikes that could cancel work

Two to four days. Deliberately before any product code.

### Spike A — R-01: can a Mac be an `ASAccessory`?

The single question that decides the iPhone product
([`01-iphone-as-source.md`](01-iphone-as-source.md) §4).

1. Write a throwaway macOS app that advertises a custom BLE service UUID via
   `CBPeripheralManager`.
2. Write a throwaway iOS app that declares that UUID in
   `NSAccessorySetupBluetoothServices` and presents the AccessorySetupKit picker
   with a matching `ASDiscoveryDescriptor`.
3. Does the Mac appear in the picker? Does pairing complete? Does the resulting
   `ASAccessory` survive a relaunch?
4. If yes, call `AccessoryNotificationCenter().requestForwarding(for:)` and see
   whether the system prompt appears on a non-EU device, and what
   `ForwardingDecision` comes back.

**Outcome:** if the Mac cannot be an accessory, the iPhone-as-source product does
not exist without shipping hardware. Record the result in this folder and in
`PROJECT_MEMORY.md` either way.

### Spike B — R-13: does the v3 QR survive an iPhone scanner?

Two hours, and it protects Phase 2 as well as Phase 4.

1. Generate a v3 pairing code with the existing Rust encoder.
2. Scan it on an iPhone with `AVCaptureMetadataOutput`, with Vision, and with
   `DataScannerViewController`.
3. Compare the recovered bytes with the encoder's output, byte for byte — the
   same assertion `CompactPairingPayloadTest` makes on Android.

**Outcome:** either the payload is unchanged, or a v4 encoding is scheduled into
Phase 2 before the Mac ships a code an iPhone cannot read.

**Gate:** both spikes answered, in writing, with the evidence.

---

## Phase 2 — The macOS desktop app

The valuable, unblocked work. Two to four weeks. Ships independently of everything
Apple-phone-shaped.

1. **Build and run.** `tauri build` for `aarch64-apple-darwin`; get the app
   launching with the front end intact under WKWebView.
2. **SQLCipher and the Keychain.** Replace DPAPI with a Keychain-held key
   ([`02-macos-desktop-port.md`](02-macos-desktop-port.md) §4–5). Arm64 only for
   v1; state it in the release notes.
3. **The listener and the permission story.** This is the phase's real work: the
   local-network prompt, running from `/Applications`, denial detection,
   diagnostics that name the cause, and a verified fallback to the relay when
   local network is refused.
4. **The loopback bridge.** Confirm early that TCC does not intercept
   `127.0.0.1:9173` (R-10). If it does, the bridge must become an in-process
   channel and that is an architectural change, not a tweak — which is exactly why
   it is tested in step 4 and not step 9.
5. **Notifications, tray, login item.** `UNUserNotificationCenter`, `NSStatusItem`,
   `SMAppService`, Mac window semantics (⌘Q quits; closing hides).
6. **Sign, notarise, staple, DMG**, plus the `codesign --verify` / `spctl` install
   check script.
7. **CI.** A real `macos-desktop-ci` job with reports uploaded on failure.

**Gate — all must hold, on real hardware:**
- An unmodified Android phone pairs with the Mac over Wi-Fi and delivers a
  notification.
- The same phone, on mobile data, delivers through the relay to the Mac.
- Disconnect holds; the reconnect switch governs both transports. (Rules R1 and R2
  from the behaviour checklist apply to the Mac exactly as to Windows, and they
  are the rules this project has broken most often.)
- The vault gate holds: nothing is shown or listed before unlock.
- A notarised DMG installs on a Mac that has never seen the app, with no Gatekeeper
  warning.
- New behaviour-checklist rows exist for every macOS-specific behaviour.

---

## Phase 3 — The Apple Rust core

One to two weeks, and it can overlap Phase 2's tail.

1. `shared/secure-channel-ffi` — the C ABI wrapper, reusing the JNI wrapper's
   handle-registry design.
2. `focusbridge_core` exposed through the same header, so iOS never reimplements
   the protocol or the QR codec.
3. The xcframework build script and a CI job that asserts every slice is present.
4. A Swift test that completes a Noise handshake against a session driven by the
   desktop's Rust code — the Apple equivalent of the Android round-trip test.

**Gate:** an xcframework that links into both a macOS and an iOS target, with a
cross-platform handshake test passing in CI.

---

## Phase 4 — The iPhone app

Only if Phase 0 said yes and Spike A came back positive. Four to eight weeks, and
the estimate is soft because three of its risks are unresolved.

1. **Companion app skeleton** — SwiftUI, AccessorySetupKit pairing, inbox, rules,
   settings, app lock. Must be useful without the accessory (a review requirement).
2. **`AccessoryTransportSecurity` extension** — key exchange with the Mac.
3. **`AccessoryDataProvider` extension** — `addNotification`, running the
   FocusBridge rules from a compact snapshot in the App Group container, returning
   `false` for anything filtered.
4. **`AccessoryTransportAppExtension`** — moving ciphertext. Bluetooth first,
   because it is the certain path; then `.localNetwork`; then `.internet`, which
   is where R-02 and R-12 get answered.
5. **Response channel** — dismissal and quick reply back through
   `sendResponse(_:)`. New protocol messages, capability-gated per
   [`05-shared-core-and-protocol.md`](05-shared-core-and-protocol.md) §3.
6. **Capability negotiation** on both ends, with the desktop UI telling the truth
   about what the paired phone can do.
7. **TestFlight**, with the EU limitation stated in the description.

**Gate:**
- An iPhone in the EU (or a test device configured accordingly) forwards a real
  notification to a Mac, over the internet, with the phone away from the Mac.
- Filtering demonstrably happens on the phone: a muted app produces no bytes.
- A quick reply typed on the Mac lands in the source app on the phone.
- Existing Android↔Windows pairs are unaffected — proven by test, not assumed.

---

## Phase 5 — Later, or never

Ranked by value, none scheduled.

| Item | Why it waits |
|---|---|
| Intel Mac (universal binary) | Straightforward; do it when someone asks |
| Mac App Store | Only if distribution demands it |
| iPhone as receiver (APNs + Notification Service Extension) | A genuinely different product with a different threat model; needs its own design document and security review ([`03-ios-app-engineering.md`](03-ios-app-engineering.md) §7) |
| iPad | The framework is iPhone-only; an iPad could still be a *receiver* |
| Apple Watch | AccessorySetupKit accessories become reachable from a companion watchOS app via CoreBluetooth **[REPORTED]** — interesting, unexplored |
| **Mac as a notification source** | **Do not build.** [`04-macos-as-source.md`](04-macos-as-source.md) |

---

## Honest estimate

| Phase | Effort | Confidence |
|---|---|---|
| 0 — decide, procure | 0.5 day + procurement | High |
| 1 — spikes | 2–4 days | High |
| 2 — macOS desktop | 2–4 weeks | Medium-high; the permission story is the variable |
| 3 — Apple Rust core | 1–2 weeks | Medium-high |
| 4 — iPhone app | 4–8 weeks | **Low** — three unresolved risks feed straight into it |

Phases 0–3 are predictable engineering against known APIs. Phase 4 is not, and its
estimate should not be trusted until Spike A and R-02 are answered.

The important structural property of this plan: **Phase 2 ships something real and
useful even if the iPhone product never happens.** Nothing in Phases 0–3 is wasted
by a negative answer to Spike A.
