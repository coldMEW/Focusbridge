# Distribution, signing and what it costs

This project has a standing rule, recorded in
[`../PROJECT_MEMORY.md`](../PROJECT_MEMORY.md): *free-first for all infrastructure
and tools; no paid resources without explicit approval.* The Apple expansion
cannot honour it. This document says exactly how much it breaks and where.

---

## 1. The unavoidable cost

**Apple Developer Program — $99 per year (or local equivalent).** **[REPORTED]**

There is no way around it for anything in this folder:

| What you want to do | Needs the paid programme? |
|---|---|
| Build and run a Mac app on your own Mac | No — free account, but a 7-day signing limit and Gatekeeper warnings |
| Distribute a Mac app to anyone else without a scary warning | **Yes** — Developer ID certificate |
| Notarise (required for a clean install on modern macOS) | **Yes** |
| Run an iOS app on your own iPhone | Free account works, 7-day provisioning |
| The three accessory entitlements | **Yes**, certainly — entitlements are attached to a paid team |
| TestFlight or the App Store | **Yes** |
| APNs keys (for the iPhone-as-receiver path) | **Yes** |

So: **$99/year buys the entire Apple expansion. Nothing else is required.** No
per-user cost, no server cost beyond the existing Cloudflare free tier, no
certificate fees on top.

That is a small number in absolute terms, but it is a decision the budget rule
says must be taken explicitly rather than absorbed. Take it explicitly.

### What free gets you

Enough to answer every open question in
[`08-risk-register.md`](08-risk-register.md) that is technical rather than
policy-shaped. A free Apple ID can build and run on a personal device with 7-day
provisioning. **[UNVERIFIED]** whether the accessory entitlements can be exercised
at all under a free account — that is R-03, and it is precisely the kind of thing
to establish before paying.

**Recommendation: do the R-01 spike on a free account first.** If a Mac cannot be
an `ASAccessory`, the $99 buys only the Mac desktop app — still worth it, but a
different decision.

---

## 2. macOS distribution

**Chosen: Developer ID + notarisation + DMG. Not the Mac App Store.**

Pipeline:

```bash
# 1. build
pnpm tauri build --target aarch64-apple-darwin --bundles app,dmg

# 2. sign (Tauri does this when configured, or do it explicitly)
codesign --force --options runtime --timestamp \
  --entitlements macos/FocusBridge.entitlements \
  --sign "Developer ID Application: <NAME> (<TEAMID>)" \
  "FocusBridge.app"

# 3. notarise
xcrun notarytool submit FocusBridge.dmg \
  --key AuthKey_XXXX.p8 --key-id XXXX --issuer <uuid> --wait

# 4. staple, so it works offline
xcrun stapler staple FocusBridge.dmg

# 5. verify what a user's Mac will see
codesign --verify --deep --strict --verbose=2 "FocusBridge.app"
spctl --assess --type execute --verbose "FocusBridge.app"
```

Steps 5 is the macOS answer to `install-desktop.ps1`. The Windows script exists
because "it did not work" and "it was never installed" looked identical
(`feafa33`, `42ca81f`); `codesign --verify` plus `spctl --assess` distinguishes
them exactly, and should be scripted rather than remembered.

### Why not the Mac App Store

- The App Sandbox needs `com.apple.security.network.server` and
  `com.apple.security.network.client`, and a listening socket on a fixed port
  bridging to itself over loopback is an invitation to review questions with no
  user benefit.
- Sandbox containers complicate the Keychain and database story for no gain.
- The Windows build already ships direct. Matching that keeps one release
  process, not two.

Revisit only if a Mac App Store presence becomes a distribution requirement.

### Hardened runtime

Required for notarisation. It forbids unsigned executable memory, DYLD injection
and unsigned libraries. Tauri and WKWebView are fine under it **[UNVERIFIED —
confirm on the first signed build; a JIT-related entitlement is occasionally
needed for webviews]**.

### Updates

Tauri's updater works on macOS and signs its own payloads. Sparkle is the
Mac-native alternative. **Recommendation: Tauri's updater**, because it is one
mechanism across Windows and macOS rather than two — and every release must still
be notarised and stapled, updater or not.

---

## 3. iOS distribution

Harder, and the options are worse than they look.

| Route | Reach | Reality |
|---|---|---|
| **App Store** | Everyone | The real answer. Review will ask about background Bluetooth, the accessory relationship and encryption compliance. All answerable. |
| **TestFlight** | 10,000 external testers, builds expire after 90 days | The right place to start, and enough for a long private beta |
| Ad-hoc | 100 devices/year, registered by UDID | Fine for personal and family use |
| Apple Developer Enterprise Programme | Internal only | Not available to a hobby project; requires a qualifying organisation and forbids public distribution |
| EU alternative marketplaces (DMA) | EU only | Requires a stand-by letter of credit or equivalent financial commitment **[UNVERIFIED — terms have changed repeatedly; check current requirements]**. Ironic given the product is EU-only, but almost certainly not worth it |
| AltStore / sideloading | Enthusiasts | Not a product distribution channel |

**Recommendation: TestFlight first, App Store when the extensions are proven.**

Note the geography again: an App Store listing is worldwide, but the notification
forwarding will only function for EU customers. The listing must say so plainly in
the description, or the reviews will say it for you. A US user downloading it and
finding the core feature inert is a one-star review that is entirely avoidable with
one honest sentence.

---

## 4. Compliance paperwork

- **`ITSAppUsesNonExemptEncryption`** must be declared. The app ships a custom
  Noise implementation, so the exemptions for "only HTTPS" do not apply. Expect to
  file an annual self-classification report. **[UNVERIFIED — confirm current US
  export requirements and whether the French declaration still applies.]** This is
  half an hour of forms, but it blocks a submission if discovered on the day.
- **Privacy nutrition labels** must be filled in for both apps. FocusBridge
  collects nothing, which makes this easy and is worth saying loudly.
- **§3.3.3(J) of the Developer Program License Agreement** governs what may be
  done with forwarded notifications. See
  [`01-iphone-as-source.md`](01-iphone-as-source.md) §5 — the existing
  architecture already complies.

---

## 5. Hardware needed

Not a licence cost, but a real one, and it must be said:

- **A Mac is mandatory.** Xcode, `codesign`, `notarytool`, `xcodebuild` and the
  simulators run nowhere else. There is no path to building or shipping either
  Apple app from the Windows machine this project currently lives on.
- **An iPhone running iOS 26.5** for anything in
  [`01-iphone-as-source.md`](01-iphone-as-source.md). The simulator cannot test
  Bluetooth accessory pairing.
- GitHub Actions `macos-latest` runners can carry CI, but not iteration.

**This is the largest practical obstacle in the whole folder**, larger than the
$99, and it should be settled before any planning goes further.

---

## 6. Total

| Item | Cost | Frequency |
|---|---|---|
| Apple Developer Program | $99 | Annual |
| Cloudflare Worker relay | $0 | Workers Free, unchanged |
| Firebase Auth | $0 | Free tier, and likely replaceable by Sign in with Apple |
| GitHub Actions macOS runners | $0 for a public repository **[UNVERIFIED]** | — |
| A Mac | Whatever a Mac costs | One-off |
| An iOS 26.5 iPhone | — | One-off |

Everything recurring, apart from the $99, stays free. The budget rule survives
almost intact; it breaks by exactly one line item, and that line item is the price
of the platform.
