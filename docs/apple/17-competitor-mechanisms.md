# iPhone companion products: mechanism comparison

Research: 2026-09-08. Baseline `cdf418f`. Documentation only.
Scope: first-party product documentation and open-source project statements.
No reverse engineering of proprietary binaries, phone test, or interoperability
proof performed. A product feature does not establish a publicly reusable API.

## What the products actually demonstrate

| Product | Evidence and mechanism | Relevance to FocusBridge |
|---|---|---|
| Microsoft Phone Link | iPhone setup requires BLE and Bluetooth permissions for notification/message features. Microsoft does not publish the complete internal implementation in its setup guide. | Nearby integration is demonstrably a product category. Build an ordinary-API feasibility spike, not a presumed clone of privileged internals. |
| Intel Unison | Intel describes mixed Wi-Fi/peer-to-peer/Bluetooth/BLE connectivity. Its iOS guide includes Bluetooth notification permission setup. Support ended January 1, 2026. | Historical evidence for feature-specific transports, not a maintained library or dependable service to integrate. |
| Dell Mobile Connect | Historical iOS guide exposes phone/Bluetooth setup and notification controls. This does not disclose the full capture implementation. | Another example to distinguish notification routing from Wi-Fi screen/file features; no justification to guess its protocol. |
| KDE Connect iOS | Project README explicitly lists cross-app notification syncing as unavailable and documents platform limitations. | Source is useful for companion-app architecture; its existence does not demonstrate Android-equivalent iPhone capture. |
| Garmin | Vendor instructions tie smart notifications to Bluetooth and the iPhone's notification settings. | Supports the accessory route; watch behavior is not proof that a Mac app can use the same OS interface. |
| Pushover | Applications/API/email integrations explicitly send messages into Pushover, which delivers to its iOS client through APNs. | Useful model for selected provider integrations. Not a universal capture mechanism for notifications already on an iPhone. |
| Pushcut Automation Server | Runs shortcuts on a dedicated iOS device and requires its app to remain in the foreground. | A dedicated appliance model, not a way to promise invisible continuous background operation on a daily-use phone. |
| AirDroid Cast | Documents iOS display mirroring via several transports and Bluetooth requirements for control. | Screen transport is not structured notification ingestion; do not count it as reliable capture/deduplication/history. |
| Apple iPhone Mirroring / notifications on Mac | Apple distinguishes nearby interactive mirroring from notification forwarding, which can continue without proximity once configured. | Important counterexample to universal proximity claims; no public third-party import API is established by that consumer feature. |

Sources for the table:

- [Microsoft requirements](https://support.microsoft.com/en-us/windows/apps/phonelink/phone-link-requirements-and-setup)
- [Intel architecture fact sheet](https://download.intel.com/newsroom/2022/2022innovation/intel-unison-fact-sheet.pdf)
- [Intel user guide](https://cdrdv2-public.intel.com/840345/Intel_Unison_UserGuide.pdf)
- [Intel end of support](https://unison.intel.com/)
- [Dell iOS guide](https://www.dell.com/support/manuals/en-us/mobile-connect/dmc_ug_ios/settings?guid=guid-08992cd2-9684-49b9-8255-578b47e6d259&lang=en-us)
- [KDE iOS source and limitations](https://github.com/KDE/kdeconnect-ios)
- [Garmin notification settings](https://support.garmin.com/en-AU/?faq=TLeDN92ZU0AgN4df6HakwA&productID=1196129&tab=topics)
- [Pushover integrations](https://pushover.net/apps), [iOS delivery](https://pushover.net/clients)
- [Pushcut operating constraints](https://www.pushcut.io/support/automation-server)
- [AirDroid Cast guide](https://www.airdroid.com/guide/cast/)
- [Apple mirroring and notification distinction](https://support.apple.com/guide/personal-safety/manage-iphone-mirroring-on-your-iphone-or-mac-ips70daa1bcf/1.0/web/1.0)

## Corrections to assumptions

Do not say every cross-network iPhone notification product is impossible:
Apple documents its own remote notification behavior. The unresolved question
is whether a third-party application can implement the needed source and
transport, with public permissions, on the targeted versions and hardware.

Do not say all notifications use the same channel as photos, calls or screen
video. A product can combine several protocols while showing one connected icon.
Our diagnostics should instead report which capabilities are actually available.

Do not infer ANCS specifically from a Bluetooth instruction alone. ANCS is an
independently documented candidate, but an undocumented competitor backend
remains undocumented. Likewise, no evidence here proves that importing another
vendor's notifications or credentials is supported.

## Practical architectures to evaluate

### A. User-authorized automation ingestion

Continue report 16's Shortcuts/App Intent experiment. This is the most useful
new software-only candidate, but content availability, exact minimum version,
permissions and background delivery are still unproven for FocusBridge.
Never expose long-lived relay secrets in a shared Shortcut. Prefer a bounded
App Intent into protected app storage and the existing authenticated delivery.

### B. Nearby Bluetooth adapter

Test ordinary Windows GATT access with a stock iPhone, independently from Mac
CoreBluetooth. Measure consent, notification add/update/remove, source metadata,
preview restrictions, action support and reconnect. Reject any implementation
that depends on spoofing Microsoft's identity or disabling OS protections.

If a public desktop API is blocked, an owned BLE development board can test an
accessory bridge. This introduces a plaintext endpoint and firmware lifecycle.
It cannot hear an iPhone that has left its Bluetooth range.

### C. Authorized EU accessory forwarding

Continue report 10's eligibility and extension proof. Keep this separate from
the automation and Bluetooth paths. Do not assume Apple's transport-security
requirements are satisfied merely because FocusBridge already uses Noise.

### D. Source-provider integrations

For a named service that has an authorized API, an integration may fetch events
directly rather than intercept notifications. This can work while the phone is
offline, but changes the product's source of truth and privacy boundaries.
Require minimum OAuth scopes, token storage, revocation, rate limits, webhook
signature validation where applicable, and provider-specific duplicate handling.

Do not suggest that every messaging provider offers such an API. Do not collect
user session cookies or reuse passwords to simulate an unsupported integration.
Provider-side capture must not be marketed as notification filtering on the phone.

### E. Coexistence with Apple notification forwarding

On an eligible Mac, users can use Apple's notifications alongside FocusBridge's
own dashboard. That may satisfy some practical needs without custom capture.
It does not put those notifications into our database or enable our filtering.
Reading Apple's private notification database or OCR of banners would create
fragile, privacy-sensitive dependencies and miss hidden or transient content.
Do not use that as the default production ingestion path.

## Product design derived from the comparison

Use a common event model with explicit adapter capabilities: origin, active
session, source identifier, optional stable notification ID, update/removal
support, action types, filtering location and history availability.

Deduplication must use source identifiers where available. Text matching cannot
reliably distinguish duplicate delivery from two legitimate identical messages.
Missing source metadata should remain missing; no fabricated contacts or apps.

Keep four independent status indicators in diagnostics: transport reachable,
peer authenticated, capture permission available, and last acknowledged event.
Only advertise per-app mute, remote actions or live history when the current
adapter can enforce them. If an automation stops firing, a healthy relay alone
must not imply healthy notification capture.

## Tests that decide whether a workaround is real

For each proposed adapter, use a repeatable synthetic workload containing:
individual messages, identical messages, updates, grouped summaries, removals,
private previews, app mute, Focus, revoked permission and actions on expired IDs.
Repeat under lock, reboot, network transition, receiver outage and burst load.
Record exact OS build, account eligibility, hardware, adapter and source app.

An adapter passes only the capabilities actually demonstrated. Integration
coverage is the intersection of source access, authorized execution and delivery,
not the union of competing products' marketing claims.

## Conclusion and open boundary

Several useful architectures exist to prototype. This review did not find a
vendor-documented mechanism giving an ordinary third-party app every requested
capability on every historical iOS version and in every country. More searching
cannot substitute for the unresolved device/API tests. Do not state that all
possible approaches have been enumerated or that universal compatibility follows.

No subscription, hardware purchase, app code change, credential change or
deployment was made. Rollback removes this document and its added index link;
the existing working Android/Windows application is untouched.
