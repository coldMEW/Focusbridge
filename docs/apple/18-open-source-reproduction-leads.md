# Open-source iPhone capture: reproduction leads

Research date: 2026-09-08. Documentation only. No dependencies installed, app
code modified, or device compatibility verified in this pass.

## Change and rollback scope

This report adds source leads and proposed experiments, not production claims.
Rollback consists only of removing this report and its report-15 index entry.
Preserve all existing user changes. No upstream code is copied; inspect licenses
before any future reuse.

## Concrete implementations

### Windows ANCS

[JPG-Consulting/IPhoneNotifications](https://github.com/JPG-Consulting/IPhoneNotifications)
describes a Windows 10 client using ANCS to display iPhone notifications as
Windows toasts. Its README requires a phone BLE helper such as LightBlue or
Aha NP to bind the computer to ANCS.

This is a stronger reproduction lead than inferring APIs from Phone Link's
marketing. It is not proof that the current Windows/iPhone combination works.
Review its GATT access and pairing implementation, then reproduce it with
synthetic notifications before adapting anything to Tauri. Check the license.

### Linux ANCS

[ancs4linux](https://github.com/pzmarzly/ancs4linux) is an iOS/iPadOS BLE
notification client for Linux. [DesktopANCS](https://github.com/schultetwin1/DesktopANCS)
also describes notification capture without an iPhone companion app.
These are implementation references, not evidence that macOS CoreBluetooth
exposes exactly the same services or authorization path.

[Tincan](https://github.com/quad341/tincan) describes a Linux companion combining
Bluetooth profiles. Treat its advertised features as upstream claims until
source review and physical reproduction establish each capability independently.
Do not infer universal social-app replies from SMS support.

### Privileged iPhone software

[kdeconnectjb](https://github.com/r58Playz/kdeconnect) explicitly targets
jailbroken/TrollStore iOS. [TrollStore](https://github.com/opa334/TrollStore)
is a privileged installation mechanism, not ordinary App Store distribution.
These repositories explain why some developer demonstrations can exceed an
ordinary app's permissions. They do not establish support for all stock iPhones.
Do not present privileged-device demonstrations as a mainstream release solution.

## Protocol constraints established by Apple

[Apple's ANCS specification](https://developer.apple.com/library/archive/documentation/CoreBluetooth/Reference/AppleNotificationCenterServiceSpecification/Specification/Specification.html)
defines added, modified and removed events and requires authorization. It also
states that service availability may change and that data/control characteristics
are optional. Consequently a robust client must handle service disappearance,
rediscovery and missing capabilities rather than equating BLE pairing with
notification delivery readiness.

## Architecture to test, not yet an implementation decision

1. Authorized iPhone ANCS source -> nearby desktop BLE adapter -> normalized
   notification events -> FocusBridge filtering/storage/UI.
2. If remote delivery is required, a receiver that remains near the phone could
   encrypt and forward events through FocusBridge's relay. This introduces an
   additional trusted endpoint and must have explicit enrollment/revocation.
3. A laptop at home cannot capture ANCS when the phone leaves Bluetooth range.
   Relaying data does not extend the original radio link. A portable gateway
   would introduce hardware, power and Internet prerequisites, not universal
   free software compatibility.
4. The Shortcuts route in report 16 remains a separate candidate for phone-side
   Internet delivery. Its input fields and execution lifecycle remain untested.

## Reproduction gates

- Record exact iOS, desktop OS, Bluetooth adapter and application versions.
- Verify discovery, explicit authorization, subscription and real event receipt
  separately; never mark capture ready on pairing alone.
- Exercise notification add/update/remove, grouped messages, hidden previews,
  non-ASCII text and long fragmented attributes.
- Test lock, reboot, Bluetooth off/on, permission revocation, radio-range loss,
  service republishing and reconnect without a helper app in the foreground.
- Confirm bounded buffers, session-scoped identifiers, no duplicate persistence,
  no notification plaintext in diagnostic logs and explicit stale-state handling.
- Test remote forwarding only after local capture passes. Record gateway
  prerequisites and trust boundaries in the UI and documentation.

## Evidence boundary

Repository existence and README claims are not executed tests. This pass adds
concrete Windows and Linux reproduction targets; it does not complete their
source audits or hardware validation. No claim of reading the entire Internet
or finding every possible future mechanism is made. Universal parity remains
unsupported; a scoped, tested notification receiver is a credible target.
