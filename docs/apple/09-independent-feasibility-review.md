# Independent Apple feasibility review

Date: 2026-09-07. Baseline: `cdf418f`. Research and proposed experiments only.
No Apple hardware, Apple entitlement approval, or platform build is demonstrated
by this document. Existing uncommitted documentation belongs to the user and is
preserved. Read this alongside the original studies, not as certification.

## Corrections that change the decision

1. **Withdraw the blanket claim that iPhone notification forwarding outside the
   EU is impossible or illegal.** Microsoft documents iPhone notification access
   on Windows via Bluetooth. That is evidence of product feasibility, not proof
   that Microsoft's complete implementation is available to an ordinary app.
   [Microsoft requirements](https://support.microsoft.com/en-us/windows/apps/phonelink/phone-link-requirements-and-setup)
2. **Do not transfer a CoreBluetooth restriction to every Windows API or BLE
   accessory.** Windows has a separate GATT stack. Its historical UWP capability
   documentation explicitly lists ANCS as unsupported, so the opposite claim,
   that any Windows app can necessarily use ANCS, is also unproven.
   [Historical capability restrictions](https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/how-to-specify-device-capabilities-for-bluetooth)
3. **A documented protocol is not an entitlement or distribution guarantee.**
   Apple's archived ANCS specification and current hardware-vendor examples
   support investigating an accessory implementation. They do not prove a
   stock macOS CoreBluetooth client can access it.
   [Apple specification](https://developer.apple.com/library/archive/documentation/CoreBluetooth/Reference/AppleNotificationCenterServiceSpecification/Specification/Specification.html)
4. **A port does not automatically preserve every feature.** Windows toast
   identity, firewall setup, launch behavior, Keychain access, and sleep recovery
   need native Mac implementations and device tests.
5. **Privacy compliance is not established by opaque relay frames alone.**
   Accessory eligibility, extensions, permitted processing/storage, destination
   binding, and user consent must be reviewed separately. Never claim compliance
   with an agreement that has not been checked against the actual implementation.

## Decision matrix

| Product path | Current conclusion | Next decisive proof |
|---|---|---|
| Android to Mac | Credible engineering port; untested here | Signed Mac build, SQLCipher migration, LAN/relay tests |
| iPhone to Windows nearby | Product category exists; FocusBridge API access unproven | Stock iPhone plus ordinary Windows GATT client |
| iPhone to Mac nearby | Native third-party access uncertain | CoreBluetooth discovery and authorized subscriptions |
| iPhone to BLE bridge to computer | Credible hardware prototype | Vendor ANCS example on an owned development board |
| iPhone directly to Internet receiver | Conditional EU accessory framework route | Eligibility, permitted receiver, signed extension proof |
| iPhone ordinary app reading all other apps | No documented general-purpose API identified | Do not build on assumed sandbox access |
| iPhone receiving FocusBridge content | Conventional companion-app possibility | APNs, privacy, lifecycle, encrypted retrieval proof |
| Mac as universal notification source | No supported universal source established | Separate investigation, not prerequisite for Mac receiver |

## Candidate A: nearby Bluetooth receiver

Prototype separately from the working networking stack. Use the Windows GATT
client APIs through a small Rust `windows` adapter or diagnostic C# application.
Record whether the packaged and unpackaged configurations differ. Access denial
is a result, not permission to spoof an OS component or disable platform security.
Microsoft documents service discovery, characteristic access, and connection
maintenance, but these general APIs do not guarantee ANCS availability.
[Windows GATT client](https://learn.microsoft.com/en-us/windows/apps/develop/devices-sensors/gatt-client)

ANCS provides added/modified/removed events and authorized attribute requests.
Its identifiers are session-scoped, replies can be fragmented, and its actions
are predetermined rather than arbitrary text replies. It is not a historical
message synchronization API. These distinctions must drive the adapter, rather
than treating every event as a new Android notification.
[ANCS contract](https://developer.apple.com/library/archive/documentation/CoreBluetooth/Reference/AppleNotificationCenterServiceSpecification/Specification/Specification.html)

Proposed adapter behavior:

- Model each BLE session with a new epoch and discard stale action handles.
- Serialize outstanding attribute requests; cap frame lengths, assembly memory,
  queue depth and timeouts before copying any remote data.
- Apply a modification to its existing active entry; removal invalidates actions.
- Separate a current-notification view from user-requested history. Resolve the
  specification's session-lifetime expectations before persisting ANCS content.
- Do not infer sender contacts or installed-app inventory from app identifiers.
- Mask the native toast as well as the React card when privacy mode is selected.
- Show Bluetooth range/loss explicitly. A relay cannot repair a missing BLE hop.

Spike acceptance: pair with explicit iPhone consent; capture an actual mail and
messaging notification; update and dismiss it; disconnect/reconnect; lock phone;
toggle notification-sharing permission; reboot both devices; test alongside
Phone Link/Apple Watch. Save metadata and timings, not private message bodies.
Pass only on named OS builds and adapter models. Repeat on a second Bluetooth
chipset before selecting a production architecture.

## Candidate B: BLE hardware bridge

Espressif publishes an ANCS client example. It is a practical starting point for
a controlled hardware experiment, not evidence of a finished consumer product.
Prefer an already-owned supported board; buying hardware is not free.
[Vendor example](https://github.com/espressif/esp-idf/blob/master/examples/bluetooth/bluedroid/ble/ble_ancs/README.md)

Two possible topologies:

1. USB bridge beside the computer: iPhone must remain nearby. Avoids reliance on
   the host's ANCS access, but adds driver/serial permission and physical-device
   management work.
2. Wi-Fi bridge beside the phone: can deliver onward over the Internet while the
   phone remains near that bridge. This is not phone-anywhere remote access.

Threat boundaries change. The bridge sees plaintext and becomes an encryption
endpoint. Require explicit ownership enrollment, physical reset semantics,
protected identity keys, authenticated firmware updates, bounded parsing,
revocation, reconnect authentication and no plaintext diagnostic output. Do not
relabel BLE link security as the existing device-to-device Noise guarantee.
Firmware licensing, radio certification, production provisioning and support
costs need evaluation before treating this as a shipping route.

## Candidate C: Apple accessory forwarding

Apple documents an iPhone accessory notification framework whose customer use
is restricted to qualifying EU devices/accounts. Development access is not the
same as customer eligibility. A general-purpose PC qualifying as the accessory
and Internet delivery under applicable terms remain explicit decision gates.
[Accessory Notifications](https://developer.apple.com/documentation/accessorynotifications)

See `10-accessory-framework-verification.md` for the independent API and policy
review. Do not flatten Apple's encrypted transport payload into the existing
Android envelope until destination binding, key ownership, extension lifecycle
and response formats have been demonstrated with a signed sample.

## Approaches that do not substitute for notification access

- Notification service extensions modify notifications addressed to their own
  app; they are not a cross-app notification listener.
  [Apple extension documentation](https://developer.apple.com/documentation/usernotifications/unnotificationserviceextension)
- Silent APNs can request background work but cannot provide guaranteed instant
  wake or continuous WebSocket execution. A visible push is not proof that app
  code ran or that a device authenticated.
  [Apple background delivery](https://developer.apple.com/documentation/usernotifications/pushing-background-updates-to-your-app)
- Email/provider integrations can deliver selected data with separate OAuth
  consent, but are not a universal iPhone notification mirror.
- Manual sharing can be useful, but must be called manual sharing.
- DNS, VPNs and alternate distribution do not by themselves grant cross-app
  notification access. A network route and permission to read data are distinct.
- Jailbreak, private APIs, pretending to be an Apple system process, location
  spoofing and unrelated background-mode abuse are not a supported production
  foundation. Do not weaken a user's phone to make a demo appear complete.

## Shared product contract before implementation

Add negotiated source capabilities rather than checking `platform == ios`:
notification source kind, current-list/history support, available action types,
app-selection authority, icon availability, filtering location, remote transport,
and consent state. Treat peer claims as information, not authorization.

Use separate states for transport reachable, authenticated peer, source
permission granted, stream active, and last verified delivery. The green status
indicator must not be driven by a relay socket alone. A disabled source should
explain its reason even when the network is healthy.

Display unsupported controls as unavailable with a brief explanation, rather
than saving rules that can never be enforced. In particular, a desktop-side
keyword filter cannot promise that blocked content never leaves an iPhone if the
content was already retrieved over Bluetooth.

## Implementation order and rollback policy

1. Freeze a baseline manifest of HEAD, dirty files and test results. Preserve
   user edits; no automatic reset, clean, database deletion or history rewrite.
2. Complete the low-risk Mac receiver port in small platform-scoped changes.
3. Run the two software Bluetooth feasibility spikes before a full iOS UI.
4. Evaluate hardware only if its added ownership/cost boundary is acceptable.
5. Validate EU accessory eligibility independently; do not make Android depend
   on that decision.
6. For each accepted code fix, document affected paths, exploit/failure scenario,
   regression test, migration implications, and exact inverse patch first.
7. Ship only measured capabilities, with signed artifacts and recovery tests.

The user requested a line-by-line security review. This research is not one.
The scoped code findings in `12-security-review-scope.md` must identify their
coverage and must not be advertised as a whole-repository security certificate.
