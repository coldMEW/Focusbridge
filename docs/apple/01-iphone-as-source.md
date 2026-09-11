# Can an iPhone be the notification source?

The question the whole Apple expansion turns on. FocusBridge's Android app calls
`NotificationListenerService`, receives every notification on the device, filters
it on the phone and forwards the survivors. This document asks what iOS offers in
its place.

---

## 1. Everything that does not work, and why

Ruling these out first matters, because each has been proposed somewhere online
as a solution and none of them is one.

| Approach | Verdict | Why |
|---|---|---|
| `UNUserNotificationCenter` | No | Sees only the calling app's own notifications. There is no cross-app read. **[VERIFIED]** |
| `UNNotificationServiceExtension` | No | Modifies **your own app's** incoming remote notifications before display. It is invoked only for pushes addressed to your app, with `mutable-content` set. Useless for other apps' notifications. **[VERIFIED]** |
| Notification Content Extension | No | Custom UI for your own app's notifications. |
| Shortcuts / Focus Filters / App Intents | No | No "any notification arrived" trigger exists. |
| Screen Time, `DeviceActivity`, `FamilyControls`, `ManagedSettings` | No | Report usage and impose restrictions. They do not expose notification content. |
| App sandbox side-channels | No | iOS sandboxing prevents an app reading another app's data by design. **[VERIFIED]** |
| MDM / supervision | No | No MDM payload grants notification-content read. Supervision changes restrictions, not APIs. **[UNVERIFIED — no primary source found asserting the negative; treated as no because no such payload is documented]** |
| **ANCS from a Mac or PC** | **No** | See §2. This is the important one. |
| Jailbreak | Out of scope | Not shippable. |

### The old answer that is gone

**ANCS — the Apple Notification Center Service** — is a Bluetooth LE GATT service
an iPhone publishes so a paired accessory can receive its notifications. It is
real, it is documented by Apple, and it is how third-party smartwatches show your
iPhone notifications. Service UUID `7905F431-B5CE-4E99-A40F-4B1E122D00D0`, with
three characteristics: **Notification Source** (`9FBF120D-…`, mandatory,
notifiable), **Control Point** (`69D1D8F3-…`, optional, write-with-response) and
**Data Source** (`22EAC6E9-…`, optional, notifiable). All require authorisation.
Only one ANCS instance exists per phone, and Apple warns it is *not guaranteed to
always be present*, so a client should watch the GATT Service Changed
characteristic for it appearing and disappearing. **[VERIFIED]**

Why it does not help FocusBridge: **consumption of ANCS through CoreBluetooth was
removed from both iOS and OS X in iOS 9** **[REPORTED]**. A Mac app cannot
subscribe to a paired iPhone's ANCS. The capability survives only for external
BLE hardware. The macOS app *Notifyr*, which did exactly this in the iOS 8 era, is
the historical proof that it once worked and the evidence that it stopped.

Building an ANCS bridge would therefore mean shipping **hardware** — a small BLE
dongle that subscribes to ANCS and relays onward. That is a different company.

### The competitors confirm the shape of the constraint

Microsoft Phone Link's iPhone support works over Bluetooth, is proximity-bound,
and is thin compared with its Android support. Intel Unison and the discontinued
Dell Mobile Connect were the same story. Nobody has iPhone notification mirroring
that works across networks, because on iOS nobody can. **[REPORTED]**

---

## 2. What does work: AccessoryNotifications

In 2026 Apple shipped a sanctioned path, under duress from the EU Digital Markets
Act, which requires Apple to give third-party devices the notification access its
own Apple Watch has.

Two frameworks:

- **AccessoryTransportExtension** — iOS 26.2, iPadOS 26.2, Mac Catalyst 26.2.
  "Transfer data securely to connected accessories that you develop." **[VERIFIED]**
- **AccessoryNotifications** — iOS 26.5. "Receive forwarded iOS system
  notifications on an accessory that you develop." **[VERIFIED]**

Apple's own description of the model:

> The Accessory Notifications framework allows accessory companion apps to request
> notification forwarding from people, and receive notification content from the
> system through an extension model. People can choose to forward notifications
> from all apps, no apps, or a subset of apps on their device. **[VERIFIED]**

"A subset of apps" is FocusBridge's per-app mute rule, enforced by iOS itself.

### 2.1 The hard constraints, quoted

From the AccessoryNotifications framework page **[VERIFIED]**:

> This framework supports iPhone only. You can develop and test an app that uses
> this framework on devices in any region. Customer installations of your app can
> only use the framework on devices located in the EU that are signed in with an
> Apple Account with an EU country or region.

From the AccessoryTransportExtension framework page **[VERIFIED]**:

> This framework is available only for iOS. The framework ignores calls for apps
> built with Mac Catalyst, and iOS apps that run on visionOS or on Macs with Apple
> silicon.

Plus, from reporting **[REPORTED]**:

- Notifications forward to **one** device at a time. Enabling a third-party
  accessory **turns off Apple Watch notifications**.
- The Settings UI (Settings → Notifications → Notification Forwarding) appears
  worldwide from iOS 26.3 but only functions for EU accounts.
- The framework API landed in 26.5; 26.3 brought the proximity-pairing and the
  settings surface.

### 2.2 What a notification actually carries

`AccessoryNotification` (iOS 26.5) **[VERIFIED]**:

| Group | Fields |
|---|---|
| Display content | `title`, `subtitle`, `body`, `summary` (Apple Intelligence summary) |
| Rich media | `sourceIcon`, `contextIcon`, `attachments` (`AccessoryNotification.File`); the body can carry a genmoji via the adaptive image glyph class |
| Interaction | `actions` (`AccessoryNotification.Action`) |
| Identity & grouping | `identifier`, `sourceName`, `threadIdentifier` |
| Timing | `deliveryDate`, `displayDate` |
| Priority | `attributes` — critical, time-sensitive, priority |

Alongside it the system passes an `AlertingContext` **[VERIFIED]** with
`shouldAlert`, `notificationCanAlert`, `isSuppressedByFocus`, a `kind` and a
`sound`. `isSuppressedByFocus` is the system telling you the user is in a Focus —
which is precisely what FocusBridge's Study Mode wants to respect rather than
guess at.

Compare that with what the Android pipeline extracts today (app name, package,
sender, message, timestamp, priority). The iOS payload is **strictly richer**:
`sourceIcon` replaces the app-inventory icon scraping entirely, `attributes` gives
real criticality instead of an inferred `UrgencyDetector` score, `summary` is free
Apple Intelligence, and `actions` is a capability the Android side does not have.

### 2.3 The response channel — the gap this closes

`NotificationResponse` and `NotificationsForwarding.Session.sendResponse(_:)`
**[VERIFIED]**. Apple's article: "If someone interacts with the notification, such
as tapping to dismiss it, or typing text in a quick reply, your accessory sends
information back to the companion app… and your app delivers it to the system."

[`../BLUEPRINT.md`](../BLUEPRINT.md) §10 lists notification actions as the one
real gap against Phone Link — "you can see a notification but not finish with it,
so the alert is still waiting on the phone". **On iOS that gap does not exist.**
The iPhone product would have a capability the Android product does not.

### 2.4 The transport ordering

`AccessoryTransport` (iOS 26.5) is an enum with `bluetooth`, `localNetwork` and
`internet`, and Apple states **[VERIFIED]**:

> The system selects the best available transport for each message by following
> this order: Bluetooth (if connected), local network (if available), then
> internet (if available).

FocusBridge already prefers the local path and falls back. Apple wrote the same
policy into the framework. The `internet` case is what preserves the cross-network
promise — the differentiator survives, at least in principle. Note carefully: the
transport extension is **your code**; the enum describes which route the system
recommends for a payload, and your extension is what actually moves the bytes.
Whether an `internet`-routed accessory is an intended and reviewable use, rather
than an artefact of the API shape, is **[UNVERIFIED]** and is risk R-02.

---

## 3. The architecture Apple requires

Three extensions, deliberately isolated from one another **[VERIFIED]**:

```
                 ┌──────────────────────────────────────────────┐
  iOS system     │  notification occurs on the iPhone           │
  notifications  └───────────────────┬──────────────────────────┘
                                     ▼
   ┌─────────────────────────────────────────────────────────────┐
   │ AccessoryDataProvider extension                             │
   │   entitlement com.apple.developer.accessory-data-provider   │
   │   EXExtensionPointIdentifier com.apple.accessory-data-provider
   │   EXCapabilities AccessoryNotifications.NotificationsForwarding
   │                                                             │
   │   addNotification(_:alertingContext:)                       │
   │     → curate: apply FocusBridge rules, drop the rest        │
   │     → serialise into an AccessoryMessage payload            │
   └───────────────────┬─────────────────────────────────────────┘
                       ▼  system encrypts, using keys from ↓
   ┌─────────────────────────────────────────────────────────────┐
   │ AccessoryTransportSecurity extension  (separate process)     │
   │   entitlement com.apple.developer.accessory-transport-security│
   │   owns the cryptographic key exchange with the accessory      │
   └───────────────────┬─────────────────────────────────────────┘
                       ▼  .ciphertext(data, featureID)
   ┌─────────────────────────────────────────────────────────────┐
   │ AccessoryTransportAppExtension                               │
   │   entitlement com.apple.developer.accessory-transport-extension│
   │   EXExtensionPointIdentifier com.apple.accessory-transport-extension
   │   dataEventHandler(event:) → move the bytes to the accessory │
   │   CANNOT decipher what it is carrying                        │
   └───────────────────┬─────────────────────────────────────────┘
                       ▼
              the accessory (a Mac, a PC) decrypts and displays
```

Apple's own summary **[VERIFIED]**: "The system coordinates these extensions,
encrypting notification data before transmission so that only your accessory can
decrypt it."

Note what this means for FocusBridge: **there would be two nested encryption
layers.** Apple's ATS-negotiated layer, which the OS insists on, and FocusBridge's
own Noise session. They are not redundant — Apple's layer protects the hop to the
accessory; the Noise layer is what makes the relay unable to read anything. Keep
both; do not try to collapse them.

### 3.1 The companion app's part

```swift
import AccessoryNotifications
import AccessorySetupKit

let accessory: ASAccessory = /* registered via AccessorySetupKit */
let center = AccessoryNotificationCenter()
let result = try await center.requestForwarding(for: accessory)
```

`requestForwarding(for:)` returns a `ForwardingDecision`: `.allow` (all applicable
apps), `.limited` (a subset the user chose), `.deny`, `.undetermined`
**[VERIFIED]**. `forwardingStatus(for:)` reads it back later, and
`presentSettings(for:scenePersistentIdentifier:)` opens the settings surface.

### 3.2 The data provider

```swift
@main
struct DataProvider: AccessoryDataProvider {
    var extensionPoint: AppExtensionPoint {
        Identifier("com.apple.accessory-data-provider")
        Implementing { NotificationsForwarding { NotificationHandler() } }
    }
}

class NotificationHandler: AccessoryNotificationsHandler {
    var session: NotificationsForwarding.Session?
    func didActivate(for session: NotificationsForwarding.Session) { self.session = session }
    func addNotification(_ n: AccessoryNotification,
                         alertingContext: AlertingContext) async throws -> Bool {
        guard alertingContext.shouldAlert else { return false }
        let message = AccessoryMessage {
            AccessoryMessage.Payload(transport: .bluetooth, data: serialise(n))
        }
        try await session?.send(message: message)
        return true
    }
    func updateNotification(_ n: AccessoryNotification) { }
    func removeNotification(identifier: AccessoryNotification.Identifier) { }
    func removeAllNotifications() { }
    func messageHandler(_ message: AccessoryMessage) { }
    func didInvalidate() { }
}
```

(Apple's own sample, abridged. **[VERIFIED]**)

`removeNotification` and `removeAllNotifications` are a gift: dismissal on the
phone propagates to the accessory, which FocusBridge currently has to model with
its own `DISMISSAL` message. `updateNotification` handles the edit case, which the
Android side does not handle at all.

### 3.3 Where FocusBridge's filtering goes

On Android, rules are enforced in `NotificationFilter` before anything is
transmitted — the privacy promise in the README is that muted apps "never leave
the phone". On iOS the same promise holds, in two layers:

1. **iOS itself** — the user's app selection in the forwarding prompt means
   unselected apps never reach the extension at all.
2. **`addNotification` returning `false`** — FocusBridge's own keyword blocks,
   Study Mode and priority rules run here, in the extension, on the phone, before
   any `AccessoryMessage` is built.

That is a *stronger* guarantee than Android's, because the first layer is enforced
by the operating system rather than by the app.

The rules themselves are already portable: `desktop/core/src/priority.rs` and
`study_mode.rs` are pure Rust with no Tauri dependency, and the same crate can be
compiled into the extension. See [`05-shared-core-and-protocol.md`](05-shared-core-and-protocol.md).

**Caution:** app extensions have tight memory limits and are killed aggressively.
Do not load the whole rules database in the extension; keep a compact, pre-compiled
rule snapshot in a shared App Group container that the companion app writes and the
extension only reads. **[UNVERIFIED — extension memory ceilings for this extension
point are not documented; treat as risk R-05.]**

---

## 4. The question that decides everything: what is an "accessory"?

Notification forwarding is granted to an `ASAccessory` — an accessory the user
paired through **AccessorySetupKit** (iOS 18+). AccessorySetupKit discovers
accessories by **[VERIFIED]**:

- **Bluetooth** — `bluetoothServiceUUID`, `bluetoothCompanyIdentifier`,
  `bluetoothManufacturerDataBlob` + mask, `bluetoothServiceDataBlob` + mask,
  `bluetoothNameSubstring`, and `bluetoothRange` (which can be restricted to
  `immediate` proximity)
- **Wi-Fi** — `ssid`, `ssidPrefix`
- **Wi-Fi Aware** — `wifiAwareServiceName`, service role, model and vendor matches

The app declares what it can set up in Info.plist via `NSAccessorySetupSupports`,
`NSAccessorySetupBluetoothServices`, `NSAccessorySetupBluetoothCompanyIdentifiers`
and `NSAccessorySetupBluetoothNames`.

So for a **Mac or a Windows PC to be the accessory**, it must be discoverable by
one of those three mechanisms.

- **Wi-Fi Aware is out.** The framework is iOS 26.0 / iPadOS 26.0 / Mac Catalyst
  26.0 — **there is no macOS row** **[VERIFIED]**. A Mac cannot publish a Wi-Fi
  Aware service. It also requires iPhone 12 or later.
- **SSID matching is out.** That describes an accessory that runs its own access
  point. A Mac is not that.
- **Bluetooth LE advertising is the only route.** A Mac can act as a BLE peripheral
  through CoreBluetooth's `CBPeripheralManager` and advertise a custom service
  UUID; Windows can do the same through `BluetoothLEAdvertisementPublisher`.
  **[UNVERIFIED — not tested, and more importantly not confirmed that Apple's
  pairing flow and review process accept a general-purpose computer as an
  accessory.]**

**This is the single unresolved question in the entire Apple expansion**, and it
is binary: if a Mac can be an `ASAccessory`, the iPhone product exists (in the EU);
if it cannot, the iPhone product requires shipping hardware and does not exist.
Do not write a line of the three extensions before settling it. The experiment is
R-01 in [`08-risk-register.md`](08-risk-register.md).

Note that BLE discovery does not doom the product to Bluetooth-only operation.
Discovery and pairing happen once, in proximity — which is exactly what scanning a
QR code is today. Afterwards the `AccessoryTransport` ordering explicitly allows
`localNetwork` and `internet`. The pairing model maps cleanly:

| FocusBridge today | iOS equivalent |
|---|---|
| Desktop shows a QR with cert fingerprint + pairing key | Mac advertises BLE with the FocusBridge service UUID; ASK picker shows it |
| Phone scans, user confirms | User taps the accessory in the ASK picker |
| Pairing session, five-minute life | ASK pairing, persistent `ASAccessory` |
| Phone pins the desktop's certificate | ATS extension negotiates keys with the Mac |
| LAN first, relay fallback | `.localNetwork` then `.internet` |

---

## 5. Apple's rules on what you may do with the content

Reported as §3.3.3(J) of the Developer Program License Agreement **[REPORTED]**.
A third party receiving forwarded notifications:

- may **not** use the information for advertising, profiling, training models, or
  monitoring location;
- may **not** disseminate it to any other application, or any other device —
  explicitly including the user's own iPhone;
- may **not** store it on cloud servers except where strictly required to deliver
  it to the accessory;
- must decrypt **only on the accessory itself**;
- may **not** alter the meaning of a notification beyond formatting for display.

FocusBridge's existing design satisfies all five without modification. Worth
stating plainly in any review correspondence:

| Rule | How FocusBridge already complies |
|---|---|
| No advertising / profiling / training | The app has no analytics, no ad SDK and no model |
| No dissemination to other apps or devices | One pairing, one accessory; `Noise_XXpsk3` binds the session to a pair ID |
| No cloud storage beyond delivery | The relay has **no queue** and stores only capability hashes |
| Decrypt only on the accessory | The relay routes opaque frames it holds no key for — by construction |
| Do not alter meaning | Rules mute or promote; they never rewrite content |

One design consequence to be careful about: "may not disseminate to any other
device" means a Mac that receives forwarded iPhone notifications **must not**
re-forward them onward — no chaining to a second PC, no pushing them back to the
phone's own inbox. The Android product's model of a single active pairing already
matches this; do not add multi-device fan-out to the iOS path.

---

## 6. What is given up versus the Android experience

| Capability | Android | iPhone (EU, iOS 26.5+) |
|---|---|---|
| Every notification on the device | Yes | Yes, for apps the user selects in the system prompt |
| Filtering on the device before transmission | Yes, in the app | Yes, in the extension **and** by iOS itself |
| App icon per notification | Scraped via app inventory | `sourceIcon`, supplied by the system — better |
| Priority / urgency | Inferred by `UrgencyDetector` | `attributes` from the system — better |
| Focus / Study Mode awareness | Own implementation | `isSuppressedByFocus` from the system — better |
| Notification actions / quick reply | **No** | **Yes** — better |
| Dismissal sync | Custom `DISMISSAL` message | `removeNotification` from the system — better |
| Cross-network, phone anywhere | Yes, proven on hardware | `internet` transport exists; **unproven** |
| Works worldwide | Yes | **No — EU only** |
| Works on old OS versions | minSdk 26 (2017) | iOS 26.5 (2026) |
| Simultaneous Apple Watch notifications | n/a | **No — mutually exclusive** |
| App inventory with icons | Yes | Not applicable; no cross-app enumeration on iOS |
| No account needed on LAN | Yes | Pairing goes through AccessorySetupKit; unchanged |

Read the middle of that table honestly: on the axes that are about *quality of the
notification*, iOS is better than Android, because the system hands you curated
data instead of making you reverse-engineer it. On the axes that are about *who
can use it*, iOS is dramatically worse.

---

## 7. Open questions

Carried into [`08-risk-register.md`](08-risk-register.md). Summarised here:

1. **R-01** Can a Mac (or PC) be registered as an `ASAccessory` through BLE
   advertising, in practice and in Apple's judgement? Decides the product.
2. **R-02** Is `AccessoryTransport.internet` usable for a general-purpose relay,
   or is it intended narrowly? Decides whether cross-network survives.
3. **R-03** Are the three entitlements freely assignable in a developer account,
   or do they require an Apple request/approval? The docs do not say either way.
4. **R-04** Does an EU Apple Account or an EU-located device gate *testing*?
   Apple says development and testing work in any region — confirm on device.
5. **R-05** Memory and lifetime limits of an `AccessoryDataProvider` extension.
6. **R-06** Does iOS 26.5 ship the framework as final, and what is the actual
   installed base among the target users?
