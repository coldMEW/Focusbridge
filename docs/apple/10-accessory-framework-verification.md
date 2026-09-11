# Accessory framework verification: eligibility is not product approval

Verified: 2026-09-07. Research only; no implementation, hardware experiment,
developer-account inspection, submission, or compliance determination performed.

Scope: independently check the accessory-framework claims in
[01](01-iphone-as-source.md), [03](03-ios-app-engineering.md),
[06](06-distribution-and-cost.md), [08](08-risk-register.md), and
[sources](sources.md). This addendum does not replace the separate ANCS/hardware
study or modify the original documents. Research was performed inline using live
Apple documentation, including Apple's documentation JSON where HTML required
JavaScript. Source IDs below link to primary pages; search snippets and third-party
reporting are not the basis for the conclusions.

## 1. Decision

**Conditional research candidate, not a confirmed shippable PC product.**
Apple documents the necessary notification-forwarding framework and non-Bluetooth
transport options. Neither an SDK declaration nor an AccessorySetupKit picker
success establishes that FocusBridge on a customer's existing PC qualifies as the
authorized accessory. No explicit acceptance or categorical exclusion of that
product was found in the primary material reviewed. Keep R-01 open, split into
technical interoperability and product eligibility. [S1][S2][S5][S11]

The appropriate next investment is a bounded eligibility/provisioning check and
disposable hardware spike, not the planned iOS app. No promise of general PC
support, cross-network reliability, universal notification actions, or automatic
compliance is justified by this research.

## 2. Versions and regional eligibility

| Component | Live Apple availability | What this establishes |
|---|---|---|
| `AccessoryNotifications` | iOS 26.5; JSON marks `beta: false` | iPhone notification API baseline, not iPad or Mac source support. [S1] |
| `AccessoryTransportExtension` | Framework metadata: iOS, iPadOS, Mac Catalyst 26.2 | Framework predates notification forwarding. Its overview explicitly limits runtime use to iOS and ignores Catalyst, iOS-on-Mac, and iOS-on-visionOS calls. [S2] |
| `AccessoryDataProvider`, `AccessoryTransportSecurity` | iOS/iPadOS/Mac Catalyst 26.5 metadata | These notification-path protocols do not make the complete feature available on all listed platforms. [S7][S8] |
| `AccessoryTransport` | iOS/iPadOS/Mac Catalyst 26.5 metadata | Transport enum availability, subject to the framework's runtime restrictions. [S4] |
| `AccessorySetupKit` | iOS/iPadOS 18.0 | Setup availability is not notification-forwarding availability. [S5] |
| `messageReceived(_:completion:)` | iOS/iPadOS/Mac Catalyst 26.5 | Current transport callback. `dataEventHandler(event:)` was introduced in 26.4 and deprecated/renamed in 26.5. [S9][S10] |

Both framework overviews specify that customer use requires the device to be
physically in the EU **and** signed into an Apple Account whose country/region is
in the EU. Both permit development and testing in any region. This is not merely
an App Store storefront, language, citizenship, purchase-country, or developer
location condition. Do not invent a travel grace period or assume a VPN changes
eligibility. Which signing/distribution configurations exercise the testing
exception remains a device/account test, especially for external TestFlight.
[S1][S2]

iOS 26.5 is a released version: Apple's security bulletin gives **May 11, 2026**
as its release date. Close the release-status half of R-29 and 01's R-06; installed
base and adoption remain unmeasured. Earlier beta appearances do not lower this
report's supported API baseline. [S12]

The runtime exclusions above concern the **Apple-side framework host**, not a
prohibition on a Mac being a remote endpoint. Conversely, they do not approve a
Mac endpoint. Avoid conflating those two roles.

## 3. Does a general-purpose PC qualify?

### What the public contract actually establishes

The companion app requests forwarding for an `ASAccessory` obtained through
AccessorySetupKit. ASK documents Bluetooth/Wi-Fi discovery, including Wi-Fi Aware
descriptors; it is not a general registration API for an arbitrary internet URL.
A matching advertisement is only an entry into onboarding. The specific endpoint
still needs working authorization, communication, and security. [S1][S5][S6]

The license defines an authorized target in terms of the developer's accessory
and explicit user authorization; the definition reviewed does not name Windows,
macOS, or a general-purpose-computer category. **Interpretation:** the absence of
a computer exclusion is not affirmative approval of software installed on
third-party hardware. [S11]

For FocusBridge, ask Apple about the exact product: an iPhone companion app plus
FocusBridge software on an existing Windows PC or Mac, no separately manufactured
accessory, desktop-owned keys, local display, and an optional opaque relay.
Identify whether the authorized target is the computer, its particular app
installation, or some other registered identity. A generic answer about BLE
accessories would not resolve this question.

### Do not import a different program's requirements

Apple's **proximity-triggered pairing** program is separately documented for EU
iPhones on iOS 26.5+. It requires organization enrollment, an Account Holder access
request, company-branded accessories intended for EU distribution/sale, an
addendum, self-certification, and Apple-server registration. That is stronger
evidence about automatic nearby pairing, not proof that ordinary in-app ASK or
notification forwarding has the same admission rules. Do not silently require
this program for the first ASK spike, or assume FocusBridge qualifies for it.
[S13]

Likewise, the word accessory alone does not prove MFi enrollment is required.
Apple distinguishes licensed accessory technologies on its accessories page.
Whether this exact product needs additional certification or approval must be
resolved explicitly, not inferred either way. [S14]

**Recommended first candidate:** BLE onboarding, followed by a real authenticated
exchange, independently on Windows and Mac. This is a test choice, not proof BLE
is the only possible route. A missing macOS SDK row does not establish every
possible peer implementation is impossible; an SSID descriptor does not establish
every computer can or cannot supply a suitable access point. This study does not
verify Windows radio APIs, drivers, or Wi-Fi Aware interoperability.

## 4. Material engineering findings

### Entitlements are necessary, availability to this team is unknown

Each extension needs its own Boolean `true` entitlement in its code signature:

| Extension | Required entitlement |
|---|---|
| Data provider | `com.apple.developer.accessory-data-provider` [S15] |
| Security | `com.apple.developer.accessory-transport-security` [S16] |
| Transport | `com.apple.developer.accessory-transport-extension` [S17] |

The entitlement pages explain requirements, not a PC-product approval process or
guaranteed issuance. Apple's public iOS capability matrix does not resolve these
three keys' membership/approval status. No authenticated portal was inspected.
Free-team eligibility, self-service versus managed issuance, distribution-profile
support, and any extra review remain open. Paying for membership is not evidence
that these gates will pass; no approval turnaround estimate is substantiated.
[S15][S16][S17][S18]

### Security is more than wrapping the existing protocol

Apple's current receiving guide requires **XWing for both `localNetwork` and
`internet`**; P256 is a fallback, not proof of network-transport readiness. The
accessory performs the key exchange and HPKE-based decryption; the documented
setup carries key material over Bluetooth. [S3][S8]

Treat Apple security interoperability as its own spike. Existing FocusBridge
Noise encryption is not a substitute. An outer Noise channel might remain useful,
but two encryption layers are an architectural choice, not an Apple requirement
or proof of policy compliance. Define endpoints and key ownership before deciding
what can be reused. Do not give the transport process notification decryption keys.

### Network transport is real; dependable remote delivery is unproven

Apple specifies Bluetooth, then local network, then internet selection according
to availability. This is more than an undocumented enum accident, but not a
provided relay service, a guaranteed permanent connection, or an execution-time
SLA. [S4]

Use the current `messageReceived(_:completion:)` path. Its documentation requires
an explicit transmission result and warns that omitting completion is treated as
success with no retry. Review `sessionInvalidated(error:)` too. One protocol page
still contains older callback examples: prefer current symbol metadata and verify
against the actual SDK rather than copying prose samples blindly. [S9][S10][S19]

### Shared state and alert semantics need corrections

The data provider's App Group container access is explicitly **read-only**. Apple
allows the companion to supply configuration, content-filter preferences, and
server tokens through it. This supports a companion-written rules snapshot; it
does not support the data provider writing history or a handoff queue there.
Other extension and Keychain access rules still require verification. [S7]

`addNotification` returns whether the accessory **alerted**, not simply whether
the extension accepted, filtered, or queued a record. A `false` result alone is
not a privacy filter: no-send behavior must be implemented before message creation.
Quiet delivery and a complete drop are distinct cases. [S20]

### Actions are conditional, not universal

Apple exposes notification actions and a response path. That establishes an API
capability, not that every source app provides a reply action, every action works
while locked, or every response succeeds. Optional content fields, icons, and
summaries also cannot be promised on every notification. Replace claims that the
action gap disappears or that the payload is strictly richer with a per-app,
per-notification capability matrix and actual response tests. [S1][S21]

## 5. Policy boundary: no automatic compliance conclusion

The live Developer Program License Agreement places forwarding-specific rules in
**3.3.7(J)**, with general privacy obligations in 3.3.3. It restricts advertising,
profiling, model training, location monitoring, onward dissemination, material
meaning changes, remote storage except for necessary delivery, and decryption
outside the authorized target. Its Forwarding Information definition also covers
derived forms. These are primary-source restrictions, not just reported ones.
[S11]

**Project recommendation, not legal clearance:** withdraw 01's statement that the
existing design complies without modification. Audit desktop history, backups,
exports, OS notification presentation, diagnostics, relay handling, and any
phone-side history against the actual authorized-target boundary. Request Apple
clarification where ambiguous. Encryption and a single pairing do not settle that
review. Do not use a decrypting dongle as an assumed workaround for an ineligible
PC. Assess any such topology separately; do not imply every hardware arrangement
is categorically forbidden. [S11]

## 6. Corrections to the earlier research

Original files remain unchanged. These are reading corrections for their named
sections, not an audit of unrelated app code or the parent's ANCS study.

| Existing location | Correction / disposition |
|---|---|
| 01 sections 2 and 4; 08 R-01 | Keep EU/26.5 baseline, but replace the single binary picker test with the separate eligibility and execution gates below. A short failed spike is not proof all PCs fail. |
| 01 sections 2.4 and 3; 03 section 4; 08 R-02 | Add XWing prerequisite and a cold remote-delivery test. Remove any inference that a foreground socket experiment settles background operation. |
| 01 diagram and sources' transport entry | The callback cited is outdated; see section 4. Apple's own examples are not fully synchronized. |
| 01 sections 2.2, 2.3, and comparison table | Actions, quick reply, summaries, icons, and source content are conditional. No universal parity or superiority claim. |
| 01 section 3.3; 03 sections 4-5; 08 R-12 | Preserve read-only rule input. Do not assume a writable data-provider handoff, shared mutable database/session state, or timely containing-app wakeup. |
| 01 section 5; sources' unconsulted agreement entry | Use the directly reviewed agreement and section 5 above; remove automatic compliance language. |
| 06 entitlement row versus its free-account caveat; 08 R-03 | Resolve the inconsistency as unknown until signed profiles and actual launches are verified. Membership alone is insufficient. |
| 06 distribution section | App Store reach is not framework eligibility. Neither TestFlight acceptance nor a debug run guarantees customer behavior. |
| 08 R-29; 01 open R-06 | Release status resolved by S12; adoption remains unknown. |
| 01 Apple Watch exclusivity statements | Not independently established by the primary pages reviewed. Retain as unverified pending exact-version Settings and delivery tests; do not infer exclusivity from alert-coordination hints. |

## 7. Concrete spike gates

All gates are **NOT RUN**. The following are proposed project acceptance tests,
not Apple guarantees. Record OS build, SDK/Xcode version, signing team/type,
profile entitlements, account region, physical region, radio/driver, and exact
endpoint build. Use synthetic notifications; exclude real content and secrets
from logs. Missing equipment or approval is BLOCKED, not PASS or product rejection.

### G0: Eligibility and provisioning, before product implementation

Ask Apple Developer Support about the exact software-on-existing-PC topology in
section 3, entitlement access, certification, and development versus customer
distribution. Separately inspect all three extension App IDs and issued profiles.
No paid enrollment or hardware purchase is authorized by this document.

**Pass evidence:** saved written scope clarification plus appropriate development
and distribution profile evidence. Keep policy and provisioning results separate;
a profile is not approval. A generic response or unresolved classification leaves
the shipping decision blocked. Disposable engineering exploration can proceed
only as explicitly bounded research, not a launch commitment.

### G1: Real PC onboarding, not picker visibility

On each candidate platform, discover a uniquely identified endpoint with ASK,
complete authorization, exchange authenticated bytes, relaunch the apps, reconnect,
remove the association, and pair again. Repeat five clean cycles on each declared
hardware/radio configuration. Do not spoof another manufacturer's identity.

**Pass evidence:** persistent correct endpoint identity and successful round trips
in all cycles, with no cross-pair delivery. Visibility alone fails the gate. A
failure must distinguish radio/driver support, descriptor mismatch, authorization,
and framework rejection. Neither Mac success nor Mac failure settles Windows.

### G2: Signed extension launch and geographic matrix

Install the signed minimal three-extension harness on a physical iPhone. Exercise
allow, selected-app, deny, and revoke flows. Verify all three extension processes
activate where needed, not merely that the permission dialog returns allow.
Test a non-EU development installation and an EU-location/EU-account customer-like
installation separately; add mismatched location/account negative controls when
legitimately available. Do not alter location signals to evade restrictions.

**Pass evidence:** profile/signature inspection, process events, correct selected
source behavior, and no forwarding after denial/revocation. If external TestFlight
classification is unresolved, record it explicitly rather than treating it as the
worldwide testing exception. No EU customer-path test means no customer-readiness
claim.

### G3: Apple cryptography and local delivery

Implement a throwaway endpoint using the documented key exchange, authenticated
decryption, and session identifiers. For network scope, prove XWing interoperation
on the actual Windows/Mac implementation, not just P256 BLE success. Test altered
ciphertext, wrong endpoint keys, session reset, and stale/replayed records.

**Pass evidence:** 100 controlled messages decrypt and display at the intended
endpoint; negative cases cause no accepted content or wrong-target output.
Record failures and reset behavior. Audit key placement. Do not wire the existing
relay/core into this spike until the Apple-only exchange is understood.

### G4: Remote operation and lifecycle, separately from G3

After onboarding, remove BLE availability and put phone and desktop on different
networks. Keep the containing app backgrounded and phone locked for a 30-minute
idle period before sending. Test cold relay connection, route loss, reconnect,
desktop sleep/wake, phone reboot after first unlock, and force-quit as distinct
states. Check both callbacks and end-to-end display, not just relay acceptance.

**Provisional threshold:** 100 supported-state messages, zero unexplained loss or
duplicate display, and p95 delivery under five seconds after connectivity is
available. These numbers are proposed spike criteria, not a proven service level.
Report tail latency, reconnect behavior, memory, and unsupported states separately.
Failure of remote delivery permits only a separately approved local-only scope;
it does not justify a background-task workaround promise.

### G5: Notification behavior and conditional actions

Use a controllable source app plus representative real apps. Cover no-action
notifications, dismiss, offered text reply, locked-phone authentication, removed
and updated records, app selection changes, Focus, quiet delivery, missing fields,
and Apple Watch coexistence. Reopen the source app to confirm any requested action
actually took effect. Test rules-snapshot refresh without a writable provider
container and ensure muted content produces no outbound record.

**Pass evidence:** a matrix of supported, unsupported, and failed behaviors, with
no fabricated reply controls or false alert-success reports. No universal actions
claim is permitted even if the tested apps pass.

### G6: Distribution and data-flow review

Review the actual end-to-end topology against section 5, including what leaves
each process, what persists, and which identity owns keys. Submit an honestly
described build and reproducible accessory instructions through the intended
distribution route when separately authorized.

**Pass evidence:** documented risk disposition, appropriate review outcome, and
successful EU customer-path testing of the exact build. No prior gate substitutes
for this one. Only then decide whether to fund an EU PC beta; production promises
still require broader reliability testing.

## 8. Primary source ledger and limitations

All successful accesses below were made on 2026-09-07. For documentation symbols,
the corresponding live JSON was retrieved using
`https://developer.apple.com/tutorials/data/documentation/<path>.json`.
Availability above comes from `metadata.platforms`, not search-result dates.

- [S1: Accessory Notifications](https://developer.apple.com/documentation/accessorynotifications): framework baseline, region and host restrictions, consent and response model.
- [S2: Accessory Transport Extension](https://developer.apple.com/documentation/accessorytransportextension): 26.2 metadata versus runtime limitations and three-extension separation.
- [S3: Receiving iOS notifications on an accessory](https://developer.apple.com/documentation/accessorytransportextension/receiving-ios-notifications-on-an-accessory): end-to-end guide; XWing requirement, key exchange, and endpoint decryption.
- [S4: AccessoryTransport](https://developer.apple.com/documentation/accessorytransportextension/accessorytransport): transport options, priority, and availability.
- [S5: AccessorySetupKit](https://developer.apple.com/documentation/accessorysetupkit): setup scope and platforms.
- [S6: ASDiscoveryDescriptor](https://developer.apple.com/documentation/accessorysetupkit/asdiscoverydescriptor): discovery traits, not PC eligibility approval.
- [S7: AccessoryDataProvider](https://developer.apple.com/documentation/accessorytransportextension/accessorydataprovider): configuration and read-only App Group access.
- [S8: AccessoryTransportSecurity](https://developer.apple.com/documentation/accessorytransportextension/accessorytransportsecurity): process separation and key-exchange responsibilities.
- [S9: Transport EventHandler](https://developer.apple.com/documentation/accessorytransportextension/accessorytransportsession/eventhandler): current and deprecated callbacks.
- [S10: messageReceived(_:completion:)](https://developer.apple.com/documentation/accessorytransportextension/accessorytransportsession/eventhandler/messagereceived(_:completion:)): completion semantics; paired with the live [deprecated callback](https://developer.apple.com/documentation/accessorytransportextension/accessorytransportsession/eventhandler/dataeventhandler(event:)) metadata.
- [S11: Apple Developer Program License Agreement](https://developer.apple.com/support/terms/apple-developer-program-license-agreement/): definitions in 1.2 and forwarding restrictions in 3.3.7(J). Public live text, not this team's accepted agreement or legal clearance.
- [S12: iOS 26.5 security release bulletin](https://support.apple.com/en-us/127110): release on May 11, 2026; no adoption evidence.
- [S13: Proximity-triggered pairing in the EU](https://developer.apple.com/proximity-pairing/): separate program's eligibility and certification requirements.
- [S14: Working with Accessories](https://developer.apple.com/accessories/): accessory-program context and design-guideline download.
- [S15: Data-provider entitlement](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.accessory-data-provider): required signed entitlement.
- [S16: Transport-security entitlement](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.accessory-transport-security): required signed entitlement.
- [S17: Transport-extension entitlement](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.accessory-transport-extension): required signed entitlement.
- [S18: Supported iOS capabilities](https://developer.apple.com/help/account/reference/supported-capabilities-ios/): membership context; does not settle these three entitlements.
- [S19: AccessoryTransportAppExtension](https://developer.apple.com/documentation/accessorytransportextension/accessorytransportappextension): protocol overview still showing older examples.
- [S20: addNotification(_:alertingContext:)](https://developer.apple.com/documentation/accessorynotifications/notificationsforwarding/accessorynotificationshandler/addnotification(_:alertingcontext:)): actual alert-result semantics.
- [S21: AccessoryNotification](https://developer.apple.com/documentation/accessorynotifications/accessorynotification): payload and optional-field declarations.

Limitations: no Apple-account access, SDK compilation, device tests, or Apple
eligibility correspondence. Apple's linked
[Accessory Design Guidelines PDF](https://developer.apple.com/accessories/Accessory-Design-Guidelines.pdf)
could not be inspected by the web tool because its approximately 39 MB response
exceeded the retrieval limit; no claim about its contents is made. That remains
an explicit eligibility-review gap. Forum posts were search leads, not official
policy or evidence that a beta-era provisioning problem persists today.

**Bottom line:** public APIs justify a carefully gated feasibility experiment.
They do not yet justify declaring FocusBridge's general-PC accessory eligible,
its relay dependable, or its existing architecture compliant.
