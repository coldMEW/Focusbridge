# 19: Verified iPhone capture routes and the revised architecture

Research date: 2026-09-08. Baseline `cdf418f`. **Documentation only — no application
code, dependency, credential, database or deployed service was changed.** Rollback
is the deletion of this file and its index links in reports 13 and 15.

> **Correction, same day.** A later research pass materially weakened §2 and §5 of this
> report. The evidence now leans strongly toward the iOS 27 notification automation being
> **trigger-and-filter only, with no notification content passed into the shortcut** — and
> the same doubt now attaches to the Message/Email triggers in §5, whose content-passing I
> asserted here too confidently. See `20-ios-app-build-blueprint.md` §1.1 and §1.2 for the
> evidence, including Apple's full WWDC26 session 310 transcript. Route B (ANCS) is
> correspondingly promoted. Read §2 and §5 below as the optimistic case, not the finding.

This report supersedes the capture-route conclusions in `00-verdict.md`,
`01-iphone-as-source.md`, `04-macos-as-source.md`, `10-accessory-framework-verification.md`
and `16-shortcuts-source-investigation.md`. Where it contradicts them, it cites a
primary source and says so explicitly. It does **not** supersede reports 11, 12 or 14.

---

## 0. The one-line answer

**Yes — capturing another app's iPhone notifications on a stock, non-jailbroken
iPhone is possible, without a jailbreak, an EU account, or Apple's permission.**

**Revised after the correction above:** of the two candidate routes, only **one is
actually established** — Route B, ANCS over Bluetooth, reproduced by third parties in
2026 against iOS 26.5 and an iOS 27 beta. It is proven on Linux, documented as closed on
macOS, and unproven-but-unblocked on Windows (experiment X2). Route A (Shortcuts) now
looks likely to be trigger-only and is **not** established.

The practical consequence: the defensible v1 is **iPhone → Windows PC, nearby, over
Bluetooth**, contingent on X2 — not the worldwide cross-network product Route A promised.

It is **not** possible to reach full Android parity on every iOS version in every
country. The precise, evidenced shortfalls are in §6. The largest are: app icons and
installed-app inventory (no route supplies them), arbitrary text reply on arbitrary
apps, and single-switch all-app capture on iOS ≤ 26 outside the EU.

A third route Apple built for exactly this purpose — `AccessoryNotifications` — is
**closed to this product**, and not for the reason the earlier reports assumed. See §3.

---

## 1. Evidence grading used in this report

| Grade | Meaning |
|---|---|
| **DOCUMENTED** | Stated in vendor primary documentation, quoted here with a URL |
| **SOURCE** | Read in the implementation's own source code or manifest |
| **REPRODUCED** | A third party reports running it against named hardware and OS builds |
| **REPORTED** | Asserted by a credible secondary source; not independently confirmed |
| **INFERENCE** | Our reasoning from the above; flagged as such |
| **UNKNOWN** | Not established; the settling experiment is named |

Nothing in this report was executed on Apple hardware. No iPhone, Mac, or BLE
adapter was available in this session. Every physical claim below is someone else's
reproduction, cited, or an experiment in §8.

---

## 2. Route A — Shortcuts notification automation → App Intent  ⭐ primary

### 2.1 The mechanism exists and it is iOS 27, not iOS 26

Report 16 flagged an unresolved iOS 26 / iOS 27 inconsistency and correctly refused
to pick a baseline. **Resolved: it is iOS 27.**

- MacRumors' iOS 27 Shortcuts guide lists the new automation types verbatim as
  `"When a notification is received"`, `"When a screenshot is captured"`,
  `"When a keyboard is connected"`, `"When an Apple Watch workout starts"`.
  [REPORTED] — https://www.macrumors.com/guide/ios-27-shortcuts/
- Neither Apple's own "What's new in Shortcuts … 26" page nor Cassinelli's iOS 26
  round-up lists a notification trigger among the iOS 26 automations. [DOCUMENTED]
  — https://support.apple.com/en-us/125148
- Two independent hands-on writers tested it on **iOS 27 / iPadOS 27 / macOS 27
  developer betas**. [REPRODUCED] —
  https://www.derekseaman.com/2026/06/home-assistant-notifications-that-run-apple-shortcuts-yes-really.html
  and https://www.derekseaman.com/2026/08/ios-27-flips-the-script-home-assistant-can-now-control-your-apple-devices.html

The WWDC26 session 310 transcript does say "In iOS 26 … three new automation types —
screenshot, keyboard connection, and notification", which is where report 16's
confusion originated. Treat that as an Apple transcript slip; the shipping-guide and
hands-on evidence both say 27, and they agree with each other.
— https://developer.apple.com/videos/play/wwdc2026/310/

**Timing that changes the plan: iOS 27 is expected to ship on or about
2026-09-14 — roughly a week from this report.** [REPORTED] —
https://9to5mac.com/2026/08/25/ios-27-release-date-when-next-major-iphone-update-is-coming/
Device support reaches back to iPhone 11 and iPhone SE 2. This route's installed
base therefore goes from zero to large within weeks, not years — the opposite of
the `AccessoryNotifications` situation report 00 described.

### 2.2 What is established about the trigger

| Property | Finding | Grade |
|---|---|---|
| Trigger wording | `"When I receive a notification from [app]"` | REPRODUCED |
| App selection | **One app per automation.** You pick a specific app; there is no "all apps" | REPRODUCED (Seaman, TWiT) |
| Filters | Up to three, on **Message, Title, Subtitle** | REPRODUCED (Seaman) |
| Runs while locked | Yes — "these Shortcut triggers can run in the background even if your device is locked" | REPRODUCED (Seaman, TWiT) |
| Runs unattended | Yes — "Run Immediately" with "Notify When Run" off | REPRODUCED |
| Regional restriction | **None found.** Shortcuts automations are not region-gated | INFERENCE (absence of any restriction in Apple's or third-party docs) |
| macOS 27 | The same three triggers land on macOS 27 | REPORTED |
| Content into the shortcut | Title / Subtitle / Body via Shortcut Input | **UNKNOWN — see 2.3** |

### 2.3 The one pivotal unknown, stated plainly

**Does the notification's content reach the shortcut as a variable, or does the
notification only *trigger* it?** Report 16 was right to insist these are different
questions, and the public record does not settle it.

What supports content-passing:

- A step-by-step guide states it explicitly: you "pass the notification title,
  subtitle and body through Shortcut Input, then map each one to a different
  field", giving a worked example that uses Subtitle, Body and Title separately.
  [REPORTED, single source, low authority] —
  https://walletpalapp.github.io/apple-shortcuts-notification-trigger.html
- The system demonstrably *has* the content, because the trigger filters on
  Message, Title and Subtitle. [REPRODUCED]
- Every comparable Apple automation trigger that has content passes it. The
  long-shipping **Message** trigger supplies the body as Shortcut Input and the
  sender as a `Sender` variable. [DOCUMENTED/REPRODUCED] —
  https://gist.github.com/squarism/27d56a857c567dea3cc9b38c0af97842

What does not support it: no Apple documentation, no WWDC statement, and no
hands-on writer confirming it. Derek Seaman's two articles demonstrate filtering
only and never show a content variable.

**Do not build on this until experiment X1 (§8) passes on hardware.** If it fails,
Route A degrades to a trigger-only signal, and the honest product becomes "an alert
that something arrived from app X", not notification mirroring. That is a materially
worse product and would make Route B primary.

### 2.4 Why an App Intent, not "Get Contents of URL"

The naive design — automation runs `Get Contents of URL` posting to a server — is
the wrong one, for reasons now supported by evidence:

1. **Shortcuts' own network actions are unreliable on a locked device.** Community
   reports are consistent that some actions simply do not run when locked, and that
   `Get Contents of URL` alone "doesn't really do anything when the phone is
   locked". [REPORTED] —
   https://talk.automators.fm/t/why-do-some-time-triggered-shortcuts-run-on-a-locked-iphone-and-others-fail/18608
2. **It would put long-lived relay credentials in shared Shortcut text.** Report 16
   already forbade this and was right.
3. **It cannot use the existing Noise transport.** The relay is a session-oriented
   binary WebSocket router, not an HTTP ingest endpoint. Adding anonymous plaintext
   ingest to make a demo work would be a security regression, not a feature.

The correct design is a **FocusBridge App Intent** the automation calls. Three
primary-source facts make this work, all newly verified in this pass:

- **App Intents run while the device is locked by default.** Apple, on
  `AppIntent.authenticationPolicy`: *"The default value of this property is
  [`alwaysAllowed`], which allows the intent to run without authentication,
  **including when the device is locked**."* And `IntentAuthenticationPolicy.alwaysAllowed`:
  *"A policy that allows the app intent to run at any time, including when the device
  is locked."* iOS 16.0+. [DOCUMENTED] —
  https://developer.apple.com/documentation/appintents/appintent/authenticationpolicy
- **Intents get 30 seconds by default**, and iOS 27 adds `LongRunningIntent` to
  extend past it: *"When a task runs in the background, the system traditionally
  gives it up to 30 seconds… incorporate this protocol and use its methods."*
  Introduced iOS/iPadOS/macOS/watchOS/visionOS **27.0, currently beta**. Requires
  regular `progress` updates or *"the system can cancel the background runtime
  extension and end your task prematurely."* [DOCUMENTED] —
  https://developer.apple.com/documentation/AppIntents/LongRunningIntent
  30 seconds is ample for one notification POST; `LongRunningIntent` is the fallback
  for draining a backlog, and it surfaces a Live Activity, so it is not free.
- **The known lock-screen App Intent failure mode does not apply to us.** The
  standing forum report of intents hanging from the lock screen is specific to
  `openAppWhenRun = true` combined with `requestDisambiguation()` — i.e. intents that
  need to show UI. Removing the disambiguation call fixes it. [REPORTED] —
  https://developer.apple.com/forums/thread/714313
  A FocusBridge ingest intent must therefore be non-interactive: no
  `openAppWhenRun`, no disambiguation, no dialog.

Two supporting mechanics for locked operation, both standard and both needing
explicit configuration:

- Keychain items must use `kSecAttrAccessibleAfterFirstUnlock` and the local
  database `NSFileProtectionCompleteUntilFirstUserAuthentication`, or the intent
  will run and then fail to read its own keys on a locked device. [DOCUMENTED
  pattern] — https://developer.apple.com/forums/thread/692355
- For delivery that survives intent expiry, a **background `URLSession`** hands the
  transfer to `nsurlsessiond`, which "will continue even if the app is suspended or
  terminated" — but set `isDiscretionary = false`, because discretionary transfers
  can be deferred for hours. [DOCUMENTED] —
  https://www.avanderlee.com/swift/urlsession-common-pitfalls-with-background-download-upload-tasks/

### 2.5 Legal boundary — this route is clean

`AccessoryNotifications`' restrictions bind only **"Forwarding Information"**, which
the Developer Program License Agreement §1.2 defines as data *"provided through or
transmitted via the Accessory Notifications Framework or Accessory Live Activities
Framework"*. Verified verbatim in the current DPLA PDF, line 99. Content a user
routes through their own Shortcut is **not** Forwarding Information, so §3.3.7(J)
does not reach it.

What does apply is ordinary DPLA §3.3.3 (Data and Privacy) — consent, disclosure,
a privacy policy, App Privacy labels, and a `PrivacyInfo.xcprivacy` manifest. All
satisfied by construction here, since the user personally authors the automation.
**No App Review guideline prohibiting this pattern was found. That is a negative
search result, not a clearance.** [UNKNOWN]

### 2.6 Route A's honest costs

- **One automation per app.** A user who wants ten apps mirrored builds ten
  automations. FocusBridge can ship a template and step-by-step guidance; it cannot
  install them silently, and must not claim a single switch. This is the biggest UX
  cost of the route and it is unavoidable.
- **No update, removal, grouping or action events.** The trigger fires on arrival.
  Dismissing a notification on the phone produces nothing. There is no action or
  reply channel back to the originating app.
- **No stable notification identifier is known to be exposed**, so deduplication is
  best-effort. Report 16's warning stands: two real messages can carry identical
  text; do not merge on text alone.
- **A feedback-loop hazard.** FocusBridge's own notifications, and Shortcuts'
  "Notify When Run" banner, are themselves notifications. Automations must never
  select FocusBridge or Shortcuts as a source, and "Notify When Run" must be off.
- **Beta-quality today.** Filtering was broken in developer beta 1 — *"For the
  automation to trigger you must not have ANY filter actions"* — and Seaman advises
  beta 5 or later. [REPORTED] Expect churn through the .0 and .1 releases.

---

## 3. Route C — AccessoryNotifications: CLOSED, and report 00 is wrong about why

Report 00 called this "the finding that changes the picture" and asserted that
FocusBridge "already satisfies every one of these [rules], by construction."
**That assertion is incorrect and must not drive implementation.**

Every payload claim in report 00 checks out. Verified against Apple's DocC JSON:
`AccessoryNotification` carries `identifier`, `sourceName`, `deliveryDate`,
`displayDate`, `title`, `subtitle`, `body`, `threadIdentifier`, `attributes`,
`summary`, `actions`, `sourceIcon`, `contextIcon`, `attachments` — the full
initializer signature. Lifecycle is complete: `addNotification`,
`updateNotification`, `removeNotification`, `removeAllNotifications`. The user can
choose "all apps, no apps, or a subset". Transport order is confirmed verbatim:
*"Bluetooth (if connected), local network (if available), then internet (if
available)."* Framework is **iOS 26.5, shipping (not beta), iPhone only**.
[all DOCUMENTED] — https://developer.apple.com/documentation/accessorynotifications
and https://developer.apple.com/documentation/accessorytransportextension/accessorytransport

The EU sentence is confirmed word for word: *"This framework supports iPhone only.
You can develop and test an app that uses this framework on devices in any region.
Customer installations of your app can only use the framework on devices located in
the EU that are signed in with an Apple Account with an EU country or region."*

**But the EU gate is not what closes this route.** The DPLA does. Corrections:

1. **The clause is §3.3.7(J), not §3.3.3(J).** Verified by downloading the current
   DPLA PDF and locating "3.3.7 Infrastructure Technologies" and the clause text.
   The §3.3.3 reference in report 00 is a widely-copied press error.
2. **The clause forbids the product FocusBridge is.** Verbatim: *"You or Your
   Application may not disseminate the Forwarding Information to any other
   Application, or any other device besides Your Authorized Target Accessory… The
   Forwarding Information may not be decrypted other than on Your Authorized Target
   Accessory."* And §1.2 defines Forwarding Information to include *"any other
   information derived from them (e.g., decrypted, hashed, re-encrypted, re-sized,
   re-formatted, summarized)."*

   A desktop notification bridge exists precisely to decrypt a notification on a PC,
   hand it to the host OS notification centre, store it in a searchable history, and
   sync it onward. Every one of those is prohibited. There is no compliant version
   of this product on this framework. It is not a workaround-able clause.
3. **"Your accessory."** §1.2 defines an Authorized Target Accessory as *"Your
   accessory"* and the transport extension as one whose *"sole purpose"* is direct
   iPhone-to-accessory transfer. A customer's existing laptop is a poor fit.
4. **`AccessoryError.unsupportedAccessory` is an undocumented veto.** Asked directly
   what an accessory must implement and whether MFi is required, an Apple engineer
   replied that *"the requirements for the accessories for implementing it are not
   yet published."* [DOCUMENTED] — https://developer.apple.com/forums/thread/817693
   So report 00's "$99 and three extensions" costing is incomplete: **MFi is UNKNOWN,
   not "not required."**
5. **Nobody has shipped on it.** No third-party product, no Apple sample code, no
   WWDC session found. A real wearable OEM (Zepp/Huami) could not even build in
   March 2026 because the two new entitlements were not provisionable in the
   Developer Portal. [DOCUMENTED] — https://developer.apple.com/forums/thread/821150

**What remains true and useful from report 00:** the AccessorySetupKit picker itself
has no MFi or embedded-hardware gate in its API — `ASDiscoveryDescriptor` needs only
a service UUID or company ID, and Apple's own sample uses a full iPhone/iPad as the
"accessory". So R-01, "can a PC be an `ASAccessory`", is probably technically *yes*
and now **moot**, because §3.3.7(J) closes the route regardless of the answer.
Retire R-01 rather than spending a Mac and a week on it.

Also note: `.internet` transport exists but is documented as the *last* fallback for
an out-of-range Bluetooth accessory, and §3.3.7(J) permits remote hops only *"as
necessary to enable delivery to Your Authorized Target Accessory"*. It is not a
licence to build a cloud relay to a desktop.

---

## 4. Route B — ANCS over BLE: alive, and report 00 is wrong here too

Reports 00 and `PROJECT_MEMORY.md` state that "ANCS consumption was removed from iOS
and OS X in iOS 9, so a Mac cannot be an ANCS client either" and that "only dedicated
third-party BLE hardware… can use it, not a Mac and not a PC."

**The first half is true and the second half is false.** The removal was specific to
Apple's own CoreBluetooth on macOS and iOS. It did not touch the protocol, and
general-purpose computers consume ANCS from stock iPhones today.

### 4.1 macOS: confirmed NO, from Apple on the record

An Apple Frameworks Engineer, 2015: *"This functionality was removed from OS X and
iOS. Neither platform can be used to consume the ANCS service anymore using
CoreBluetooth,"* clarified in 2016 as *"neither iOS 9 nor OS X 10.11 can CONSUME
ANCS any longer."* And in 2017: *"iOS apps cannot create or consume ANCS services.
This is by design."* [DOCUMENTED] — https://developer.apple.com/forums/thread/24336
and https://developer.apple.com/forums/thread/92557

These are the newest primary statements found; no one has published a successful
macOS CoreBluetooth ANCS test since 2017. Experiment X4 (§8) would refute or confirm
it cheaply, but plan for NO.

### 4.2 Linux: REPRODUCED against iOS 26.5 and an iOS 27 beta, in 2026

This is the finding that reopens the route.

- **tincan** (MIT, actively developed through 2026-07) states its reference setup as
  *"iPhone (iOS 26.x) ↔ Fedora 44, BlueZ 5.86"* and lists *"✅ App notification
  mirroring (ANCS) — see notifications from phone apps on your desktop, with per-app
  filtering"* — explicitly *"no jailbreak, no Apple-ID risk"*. [REPRODUCED, verified
  by reading the README directly] — https://github.com/quad341/tincan
- **blueferry** states: *"Most development has used an iPhone 16 Pro Max on iOS 26.5,
  with additional successful testing on an iPhone 17 Pro Max running an iOS 27
  beta,"* and *"optionally mirror other iPhone notifications… There is no Mac relay,
  Apple login, cloud service, or subscription."* Ships distro packages.
  [REPRODUCED, verified directly] — https://github.com/erikwb/blueferry
- Its `PROTOCOL.md` is the most detailed public field report available and documents
  the real failure modes: the bond must be initiated **by the iPhone**, or some
  controllers produce a single-transport bond that prevents ANCS access; the first
  Control Point write returns `NotPermitted` until the user taps the consent prompt
  ~5 s later; `StopNotify`/`StartNotify` flapping SIGSEGVs bluetoothd 5.87.

The mechanism is: advertise as a BLE peripheral with a **Service Solicitation AD
field (type `0x15`) naming the ANCS UUID**, let the iPhone connect, bond, accept the
"Show Notifications" prompt, then subscribe as an ordinary GATT client. No companion
iOS app. No MFi — Apple's MFi FAQ carves out BLE-only accessories.

Two environmental gotchas worth recording now: BlueZ gates `SolicitUUIDs` behind
`--experimental`, and BlueZ ≤ 5.86 has a bug that breaks *all* LE advertising on
Linux ≥ 7.0 kernels (fixed by commit `2a6968b4`, 2026-06-02). Controller choice also
matters — an RTL8761B worked where a MediaTek MT7925 did not.

### 4.3 Windows: one specific, documented blocker

Windows has **no ANCS capability restriction.** The folklore is wrong. The archived
UWP capability docs contain no ANCS entry, and the only Windows ANCS client that
ever shipped declared nothing but `<DeviceCapability Name="bluetooth" />` in its
manifest. [SOURCE] — https://github.com/JPG-Consulting/IPhoneNotifications
(That project is dead: last commit 2016-12-01, **no LICENSE file — all rights
reserved, not reusable**, and an open issue says it broke on Windows 10 1709.)

The real blocker is narrower and documented: **Windows forbids publishing the
solicitation advertisement.** `BluetoothLEAdvertisementPublisher` lists as
system-reserved and not allowed *"List of 128-bit Service Solicitation UUIDs
(0x15)"*. [DOCUMENTED] —
https://learn.microsoft.com/en-us/uwp/api/windows.devices.bluetooth.advertisement.bluetoothleadvertisementpublisher

So the clean path Linux uses is unavailable to any Windows app. That leaves the
question experiment **X2** must answer: does pairing an iPhone through Windows
Settings in 2026 yield an LE bond under which `GetGattServicesForUuidAsync(ANCS)`
returns the service? Phone Link and the old Dell Mobile Connect both prove the
*stack* can do it — Dell's manual instructs users to enable *"Show Notifications"*
in the iPhone's Bluetooth settings, which is the ANCS consent gate, and Dell Mobile
Connect was a third-party app. Whether the public WinRT surface exposes it to us is
**UNKNOWN** and is the single highest-value cheap experiment in this report.

### 4.4 What ANCS does and does not give

| Capability | ANCS |
|---|---|
| Add / **modify** / **remove** events | **Yes** — EventID 0/1/2 |
| App bundle identifier | Yes — `AppIdentifier` |
| App display name | Yes — `GetAppAttributes` → `DisplayName` |
| **App icon** | **No.** `AppAttributeID` defines exactly one value. No icon, colour or badge exists anywhere in the protocol |
| Title / Subtitle / Message / Date | Yes |
| Actions | **Only two** — Positive and Negative, when the corresponding EventFlag is set. No arbitrary reply, no custom buttons |
| Companion iOS app | **Not required** |
| Category | Yes — 12 values (IncomingCall, Email, Social, …) — a real gain for FocusBridge's categorisation |
| Content while locked | Degrades with Show Previews. tincan's docs: *"If the user sets previews to 'When Unlocked' or 'Never,' the Message attribute is correspondingly abbreviated/empty"* |
| Session semantics | NotificationUIDs and AppIdentifiers are **session-scoped**; discard on disconnect. Existing notifications replay with `PreExisting` set |
| Service stability | Apple: *"the ANCS is not guaranteed to always be present."* A client **must** subscribe to GATT Service Changed |

Range is the hard limit: a laptop at home cannot hear an iPhone that has left
Bluetooth range, and no relay repairs a missing radio hop. Report 18 was right.

---

## 5. Route D — the pre-iOS-27 fallback that already works everywhere

Worth stating because it is shipping, worldwide, on old iOS, and the earlier reports
overlooked it: the **Message** and **Email** communication automation triggers have
existed for years and run unattended.

**Corrected claim.** I originally wrote that they *"do pass content — the message body as
Shortcut Input and the sender as a `Sender` variable"* and graded it DOCUMENTED. **That
grade was wrong.** Apple's page describes both purely as *filters*: *"Use a communication
trigger to run an automation when you receive an email or message… all criteria must be
met."* The options are `Sender` / `Subject Contains` / `Account` / `Recipient` for email
and `Sender` / `Message Contains` for messages, each worded as *"Triggers your automation
when you receive…"*. Nothing there produces a variable.
[DOCUMENTED] — https://support.apple.com/guide/shortcuts/communication-triggers-apdd711f9dff/ios

Community SMS-forwarding shortcuts do appear to move message text, so this is genuinely
contested rather than settled either way. **Grade it UNKNOWN and test it (experiment X6)
before promising this tier.**

This covers SMS, iMessage and Mail — for most users the two highest-value
notification classes — on iOS long before 27, in every region, with no entitlement.
It should ship as Route A's compatibility tier, labelled for exactly what it is.

---

## 6. Compatibility matrix

**Capture (phone side)**

| Route | Min iOS | Region | Apps covered | Setup per app | Locked | Update/remove | Actions | Icons |
|---|---|---|---|---|---|---|---|---|
| A — Shortcuts notification automation | **27** (~2026-09-14) | Worldwide | Any app the user picks | **Yes, one automation each** | Yes | No | No | No |
| D — Message/Mail triggers | ~13+ | Worldwide | Messages, Mail only | One each, total | Yes | No | No | No |
| B — ANCS over BLE | Any current (REPRODUCED on 26.5 / 27β) | Worldwide | **All apps, one consent** | None | Yes (content varies with Show Previews) | **Yes** | 2 only | **No** |
| C — AccessoryNotifications | 26.5 | **EU device + EU Apple Account** | All / subset | None | Yes | Yes | Yes + reply | Yes |

Route C's column is complete and irrelevant: **§3.3.7(J) forbids the product.**

**Receive (desktop side)**

| Desktop | Route A / D | Route B (ANCS) |
|---|---|---|
| Windows 10/11 | **Yes** — existing Tauri receiver, unchanged | **UNKNOWN** — blocked on X2; 0x15 publishing is forbidden |
| macOS | **Yes** — after the port work in report 11 | **No** — Apple removed CoreBluetooth ANCS consumption |
| Linux | Yes | **Yes** — REPRODUCED (tincan, blueferry) |

**Transport.** Routes A and D deliver over the existing LAN and Cloudflare relay
paths and therefore work **across networks**, worldwide, with the phone anywhere.
Route B is Bluetooth-range-bound at the capture hop; a relay extends delivery, never
the radio link.

**Mac as a notification *source*.** Report 00 called this fragile and App-Store-hostile
because it required reading an undocumented database behind Full Disk Access. That is
no longer the only option: the notification automation trigger reportedly lands on
**macOS 27** too, giving a sanctioned, permissioned path with the same constraints as
Route A. [REPORTED] Re-open `04-macos-as-source.md` after X1.

**Costs.** Routes A and D need an **Apple Developer Program membership, $99/year**
(required to ship any iOS app) and a Mac to build on. No hosting cost — the existing
free-tier Worker carries it. No hardware. Route B on Windows/Linux needs no
membership at all and no hardware beyond a supported Bluetooth adapter. A BLE
microcontroller bridge remains available as a hardware fallback (ESP-IDF `ble_ancs`
and Nordic's ANCS Client are both maintained, MFi-free) but introduces a plaintext
trust boundary and a firmware lifecycle — treat it as a separate product.

---

## 7. Revised architecture

The shape that survives all of the above is **one normalized event model with
explicitly negotiated adapter capabilities** — which report 09 already prescribed and
which the code does not yet have.

```
CAPTURE (phone)                 PROCESS + STORE (phone)          TRANSPORT            DESKTOP
────────────────                ────────────────────────         ─────────            ───────
A. Shortcuts notification  ┐
   automation (iOS 27, WW)  │
D. Message / Mail trigger   ├──▶ FocusBridge App Intent      ┐
   (iOS 13+, WW)            │    · authenticationPolicy       │
                            │      = .alwaysAllowed           │
                            │    · no openAppWhenRun,         │
                            │      no disambiguation          │
                            │    · bounded, untrusted input   │
                            │    · versioned rule snapshot    ├──▶ existing Noise ──▶ existing Tauri
                            │      evaluated ON PHONE         │    XXpsk3 session      receiver
                            │    · write event to encrypted   │    over LAN (pinned    (Windows today,
                            │      store FIRST, then deliver  │    TLS) or Cloudflare   macOS after
                            │      · Keychain AfterFirstUnlock│    relay                report 11)
                            │      · file protection          │
                            │        CompleteUntilFirstUnlock │
                            │    · retry unsent RECORDS,      │
                            │      never stale ciphertext     │
                            │    · delivered only on app-level│
                            │      ACK from intended peer     ┘
B. ANCS over BLE ───────────────────────────────────────────────▶ desktop-side adapter
   (desktop is the consumer; no phone-side app at all)             (Linux proven,
                                                                    Windows pending X2)
```

Five design commitments this forces, each traceable to evidence above:

1. **Filtering happens on the phone, in the intent, before storage.** A desktop-side
   keyword filter cannot promise blocked content never left the phone. Report 09 said
   this; Route A makes it enforceable, because the intent is the only ingress.
2. **Capability negotiation replaces `platform == ios`.** An adapter advertises:
   source kind, whether it supplies update/remove, which actions exist, whether it
   can enumerate installed apps, whether icons are available, where filtering runs.
   The UI shows unsupported controls as unavailable with a reason — it must not save
   a per-app mute rule that Route A can never enforce, or offer Reply where no route
   supplies one.
3. **Four independent status lights, not one.** Transport reachable · peer
   authenticated · capture permission present · last acknowledged event. A healthy
   relay must never imply healthy capture — if the user deletes an automation,
   FocusBridge must say capture stopped, not show green.
4. **A typed, validated notification contract.** Today there is none: Kotlin
   hand-builds JSON, the desktop plucks fields from an untyped `serde_json::Value`
   with silent defaults (a missing `appName` becomes `"Unknown"`, a missing `id` is
   invented), and `shared/protocol.json` is stale. Adding a third and fourth client
   to that is the largest structural risk in this plan. Define the event once in
   `desktop/core`, deserialize rather than pluck, reject malformed input.
5. **The connection rules apply unchanged.** An iOS source honours rule 1 (the
   phone's reconnect toggle is absolute) and rule 2 (disconnect means disconnect,
   decided at authentication next to the setting, never next to the transport).
   Route B is the exception worth flagging early: an ANCS bond is owned by the OS
   Bluetooth stack, so "disconnect" there means dropping our GATT subscription and
   refusing to re-subscribe — the bond itself survives, and the UI must say so.

---

## 8. Experiments, with pass/fail criteria

Ordered by value per unit of cost. **X1 decides the product.**

**X1 — Does the notification automation pass content? (iOS 27 iPhone, ~1 hour)**
Prereq: any iPhone on iOS 27. Build a notification automation for a developer-controlled
test app. Send synthetic notifications with unique tags in title, subtitle and body.
In the shortcut, add `Show Notification` / `Add to Text File` and attempt to insert
Shortcut Input and every offered variable.
*Pass:* title, subtitle and body are individually retrievable as variables.
*Partial:* only some fields, or one concatenated blob — record exactly which.
*Fail:* no content variable exists → Route A is trigger-only; make Route B primary
and re-scope the product before writing any iOS code.
Then repeat under: locked, Show Previews = Never, Focus on, Low Power Mode, after
reboot, after force-quitting FocusBridge, and a 20-notification burst. Record what
arrives, in what order, and what is dropped.

**X2 — Windows ANCS GATT access (Windows 11 + iPhone, ~2 hours)**
Pair the iPhone via Settings → Bluetooth. From an **unpackaged** .NET console app,
standard user token, no manifest:
`BluetoothLEDevice.FromBluetoothAddressAsync` → `GetGattServicesForUuidAsync(7905F431-B5CE-4E99-A40F-4B1E122D00D0, Uncached)`
→ `GetCharacteristicsForUuidAsync(9FBF120D-…)` → CCCD write Notify.
*Pass:* the service resolves, the CCCD write returns `Success`, iOS raises the
"display your iPhone notifications" prompt, and Notification Source events arrive.
*Fail:* record the exact failure — null device, absent service, `AccessDenied`,
`Unreachable` — as a result, not a licence to spoof Microsoft's identity or disable
platform security.
Run twice: with Phone Link installed and connected, and with it removed, to settle
whether ANCS subscription is exclusive.

**X3 — App Intent while locked (iOS 27 iPhone, ~2 hours; depends on X1)**
A minimal intent with `authenticationPolicy = .alwaysAllowed`, no `openAppWhenRun`,
no disambiguation. It writes a timestamped row to a file-protected store and POSTs
to a local listener.
*Pass:* with the phone locked for 10 minutes, every invocation writes its row and
delivers, with keys read successfully from Keychain.
*Fail:* note precisely whether the intent did not run, ran but could not read keys
(→ wrong protection class), or ran but could not reach the network (→ background
`URLSession` with `isDiscretionary = false`).

**X4 — macOS ANCS refutation (Mac + iPhone, ~1 hour)**
CoreBluetooth central against a bonded iPhone; `discoverServices(nil)` then
`discoverServices([CBUUID(string: "7905F431-…")])`.
*Expected:* absent both times, confirming Apple's 2015 statement in 2026. Publish the
result either way; nobody has since 2017.

**X5 — Linux ANCS reproduction (Linux box + iPhone, ~3 hours)**
Reproduce tincan or blueferry end-to-end on a patched BlueZ with a known-good
controller. This is the cheapest way to obtain a *known-working* ANCS reference to
test a Windows or Mac client against, and it de-risks X2's failure interpretation.
*Pass:* add, modify and remove events for a real Mail and Messages notification, plus
one Positive action performed successfully.

**X6 — Route D baseline (any iPhone, ~30 minutes)**
Confirm the Message trigger still supplies body and `Sender` on current iOS, locked
and unlocked. Cheap, and it establishes the compatibility tier independently of X1.

**Standing test matrix for any adapter that passes.** Per report 17, and unchanged:
individual messages, identical messages, updates, grouped summaries, removals,
private previews, app mute, Focus, revoked permission, actions on expired IDs —
repeated under lock, reboot, network transition, receiver outage and burst load.
Record exact OS build, hardware, adapter and source app. **An adapter passes only
the capabilities it actually demonstrated.**

---

## 9. Remaining blockers

| # | Blocker | What settles it |
|---|---|---|
| B1 | Notification content may not reach the shortcut | **X1.** No amount of further searching substitutes |
| B2 | Windows ANCS GATT access unproven | **X2**, with X5 as the control |
| B3 | Route A burst/rate-limit behaviour unknown | X1's burst step |
| B4 | No App Review precedent for this pattern | Ship a TestFlight build; or ask App Review directly. Absence of a prohibition is not clearance |
| B5 | macOS 27 notification trigger only REPORTED | Re-run X1 on macOS 27 |
| B6 | Untyped notification contract across three clients | Engineering, not research — item 4 in §7 |
| B7 | Desktop Rust and Noise test suites never run in CI | Engineering. Adding two clients to an ungated suite is the process risk to fix first |

Retired: **R-01 (can a PC be an `ASAccessory`)** — moot, see §3.

---

## 10. Phased plan

**Phase 0 — before any Apple work.** Fix the two open security findings in report 12
(F1 enrollment arming, F2 unrevoked relay capability on delete), each with the
integration regression report 12 specifies. Turn on `cargo test` for the desktop and
`shared/secure-channel` in CI on all three OSes; today macOS is `cargo check` only
and the Noise crate is never tested at all. Add behaviour-checklist rows for both.

**Phase 1 — macOS receiver.** Independent of every iPhone question and the highest
certain value. Report 11 governs. The concrete items are: replace `hostname -I` in
`pairing_cmd.rs` with real interface enumeration (that flag is GNU-only and silently
returns nothing on macOS today — a live bug), add the macOS 15 local-network privacy
entitlement and usage description, exercise the already-written Keychain path, and
sign + notarize.

**Phase 2 — run X1 and X2 in parallel.** One is an iPhone afternoon, the other a
Windows afternoon. They decide the entire iPhone product between them. Do not write
an iOS app first.

**Phase 3 — build to the result.**

- *X1 passes:* ship Route A + Route D as one iOS app: bounded App Intent, on-phone
  filtering, encrypted store, existing Noise transport, capability negotiation, and
  a guided per-app automation setup that is honest about being per-app.
- *X1 fails, X2 passes:* Route B becomes primary on Windows. A different, narrower
  product — nearby-only, no icons, two actions — but with update/remove and one-time
  all-app consent, which Route A cannot offer.
- *Both fail:* ship Route D (Messages and Mail) as the honest iOS tier and say so
  plainly. Do not invent coverage to fill the UI.

**Fallbacks, documented rather than discovered later.** Provider integrations with
their own OAuth for named services (different source of truth, different privacy
boundary — never marketed as on-phone notification filtering). A BLE microcontroller
bridge if desktop ANCS access fails everywhere. Coexistence with Apple's own iPhone
Notifications on Mac, which — verified — needs no proximity at all: *"Your iPhone
must be turned on, but it doesn't need to be nearby,"* and works *"even when iPhone
Mirroring is not in use."* [DOCUMENTED] — https://support.apple.com/en-us/120684
That satisfies some users outright, and it remains unavailable in the EU, so the
regional complementarity report 00 identified is real even though its preferred
route is not.

---

## 11. Evidence boundary

This is a component-level research report, not a certification and not a whole-repo
audit. No Apple hardware, no BLE adapter and no iOS 27 device was available. Every
physical claim is a cited third-party reproduction or an experiment in §8. No claim
is made that the entire Internet was searched or that every future mechanism has
been enumerated. Universal parity across all iOS versions and regions remains
unestablished, and §6 states exactly which requirements prevent it.

Licences checked before any reuse proposal: **tincan MIT**, **blueferry** (packaged
releases; confirm SPDX before reuse), **ancs4linux MIT** (relicensed 2026-08-29 —
pin the commit), **ios-notif-forward MIT**, **IPhoneNotifications no licence — do
not reuse**, **DesktopANCS no licence — do not reuse**, **ForwardNotifier GPL-3.0
and jailbreak-only (iOS 11–13)**. Read these as protocol references; copy nothing
without a second licence check at the commit you take.
