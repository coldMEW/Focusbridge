# New candidate: user-authorized Shortcuts notification ingestion

Research date: 2026-09-08. Documentation only; no app or live data changes.

## New primary evidence and correction

Apple's WWDC26 Shortcuts session describes an automation triggered by a
notification from a selected app, with keyword filtering. This contradicts
the categorical rejection of notification-triggered Shortcuts in report 01.
It establishes a trigger, not FocusBridge-compatible delivery or full parity.
[Apple session, Automations chapter](https://developer.apple.com/videos/play/wwdc2026/310/)

Version caution: the WWDC26 page's chapter summary mentions iOS 26, while
the surrounding feature reporting refers to iOS 27. Do not choose a minimum
version from that inconsistent metadata. Verify the actual OS build, trigger
availability and supported input fields on hardware before setting a baseline.
The session does not establish worldwide availability or old-version support.

## Why investigate this before building an accessory product

Engineering hypothesis: a user creates an automation for selected notification
sources and invokes a FocusBridge App Intent. If the required content is exposed,
the intent filters and records it locally, then attempts authenticated encrypted
delivery. This could avoid needing an accessory for that limited ingestion path.
It does not give FocusBridge a system notification-listener entitlement.

The user must own and authorize the automation. Do not silently install it,
pretend permissions were granted, or ask them to weaken device security. Do not
promise a single switch for all apps before proving that configuration exists.

## First experiment: content and permissions, no server

On a spare compatible physical iPhone:

1. Record hardware, OS build, region, Shortcuts settings and source permissions.
2. Create a notification automation for a test app controlled by the developer.
3. Inspect the automation input with synthetic, uniquely tagged notifications.
4. Record which fields are available: app identity, title, subtitle, body, time,
   identifier, grouping, attachments and any action metadata. Mark absent fields
   absent; do not manufacture identifiers or content from unrelated data.
5. Repeat with user-approved mail and messaging sources, using synthetic content.
6. Test locked/unlocked, preview hidden/visible, Focus, Low Power Mode, reboot,
   app force quit, permission revocation, and a notification burst.
7. Test trigger-only behavior separately from permission to pass content into
   another app's action. Do not infer one from the other.

Stop this path if content cannot be supplied safely or execution requires an
interaction incompatible with the chosen product. Preserve a useful manual mode
if desired, but label it manual rather than silently advertising automatic sync.

## Second experiment: bounded local App Intent

Proposed interface: `Import notification into FocusBridge`. Accept only the
fields established by experiment one. Set explicit limits on text and attachment
sizes. Treat every input as untrusted, including a user-edited Shortcut.

The action should evaluate a versioned rule snapshot and write a bounded record
to encrypted app-controlled storage. Reject unsupported formats without exposing
content in errors. Avoid opening the main UI unless the OS or user requires it.
Measure whether locked-device key access and intent execution actually work.

Do not place device private keys, relay bearer credentials or PSKs in shared
Shortcut text, URLs or exported automation definitions. Use a narrow App Intent
boundary into platform-protected app storage. Validate that action invocation
cannot bypass local user consent or an explicit sync disconnect.

## Third experiment: transport and durability

The existing relay is a session-oriented binary WebSocket router, not a public
HTTP notification-ingestion endpoint. A Shortcut HTTP POST cannot simply replace
the current authenticated Noise protocol. Never add anonymous plaintext ingest
to make the prototype appear functional.

Proposed sequence:

- Persist an application event before attempting network delivery.
- Establish a fresh authenticated device session when execution time permits.
- Retry unsent application records, never ciphertext from a previous session.
- Mark delivered only on a matching application ACK from the intended receiver.
- If execution expires, expose pending status and retry only during authorized
  future execution. Do not promise the OS will run the action again on demand.

An asynchronous encrypted mailbox is a possible separate design if short-lived
execution cannot sustain the current session. It would add key distribution,
offline replay prevention, retention, server storage, abuse limits and privacy
obligations. It is not an incidental relay tweak and requires separate review.

## What remains unsupported until separately proven

| Requirement | Evidence still required |
|---|---|
| Every historical iOS version | A new automation does not backport itself |
| Every country | Regional/device availability tests, not absence of an EU warning |
| Every installed app | Selection mechanics and inaccessible-source behavior |
| Full message bodies | Input fields under lock-screen privacy settings |
| Delete/update synchronization | Explicit lifecycle events, not just arrival triggers |
| Stable deduplication | Stable identifiers or documented weaker semantics |
| Reply/dismiss on original app | An authorized action channel, separate from capture |
| Accurate app inventory/icons | A supported inventory source, not observed notifications |
| Instant PC-triggered wake | A supported wake mechanism with measured constraints |
| Zero missed messages | Cannot infer this from a finite successful test |

Do not merge identical text indiscriminately: two real messages can have the same
body. Without source identifiers, disclose best-effort duplicate handling and
retain enough local provenance to investigate without logging private content.

## Complementary paths, not magic replacements

Older systems may still use proven Bluetooth accessory support, selected
provider integrations or manual sharing. Each has different permissions and
coverage. Keep capability negotiation explicit rather than presenting all modes
as the same product. The EU accessory framework remains an independent route.

Apple provides an interoperability request process for eligible developer-program
members. A precise request about source access, desktop accessory eligibility or
missing lifecycle callbacks is more useful than depending on an undocumented
bypass. Submission does not guarantee approval, a schedule or worldwide rollout.
[Apple interoperability process](https://developer.apple.com/support/interoperability-requests)

## Acceptance and rollback

Proceed to implementation only after the first experiment answers content,
permission and lifecycle questions. Record the exact supported build instead of
using an unverified minimum version. Keep this adapter optional and unable to
alter existing Android/Windows pairing or keys.

This document corrects research, not code. Rollback removes only this new report
and its index/checkpoint links. No database, dependency, live credentials or
production service was changed. Universal support remains unestablished.
