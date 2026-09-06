# Phone Link comparison and FocusBridge connection architecture

Reviewed: 2026-09-05. This is a source-backed comparison and implementation
contract, not a claim that the proposed functionality is already available.
Microsoft's public support documentation describes product behavior, not its
complete private server implementation or cryptographic protocol.

## What "anywhere" actually means

Phone Link permits mobile-data content sync when enabled. Microsoft still
documents LAN isolation, battery-saving behavior, firewalls, and public networks
as possible obstacles. Calling requires Bluetooth; Apps/phone-screen features
have same-Wi-Fi and supported-device requirements. There is no documented promise
that every feature works on every network or every phone.

Sources: [connectivity troubleshooting](https://support.microsoft.com/en-us/windows/apps/phonelink/troubleshooting-the-phone-link),
[calls](https://support.microsoft.com/en-us/windows/apps/phonelink/setting-up-calls-in-the-phone-link),
[Apps troubleshooting](https://support.microsoft.com/en-us/windows/apps/phonelink/troubleshoot-apps-in-the-phone-link).

## Feature comparison

"Present" below means implemented in the examined source, not certified on every
device. The confirmed physical test device is a Pixel 7 running Android 17, with
a Windows desktop on a reachable local Wi-Fi network.

| Capability | Phone Link public behavior | FocusBridge current state and required work |
| --- | --- | --- |
| QR and manual pairing | Account-linked onboarding and phone permission prompts | LAN QR/manual payload and pinned certificate present; remote rendezvous absent |
| Saved devices | Account device list, additional PCs require authorization | Stable phone install ID/name/time present; local list is not cloud presence |
| Different-network sync | Mobile-data sync setting | Blocked until secure relay client/server protocol is ready; desktop relay client is a stub |
| Reconnect from PC | Authorized account-linked device workflow | Current request uses the existing socket, so cannot reach a disconnected phone |
| Notifications | Capture, display, app selection | Present, with ACK queue; real app-specific parsing and background acceptance still need broader testing |
| Notification actions | Supported replies and actions; dismissals propagate to Android | No complete RemoteInput/action pipeline; desktop delete currently deletes local history only |
| SMS/MMS | Read/send; device-dependent RCS support | Notification mirroring is not SMS access; no SMS/MMS backend |
| Calling | Bluetooth calling and recent calls | No call audio/dialer backend; ongoing call notifications are filtered, so do not promise call coverage |
| App list and app control | Supported-device app mirroring and launch | Inventory names/icons/categories and filtering rules present; not remote app execution |
| Photos/files | Windows 11 photo access moving to File Explorer; file sharing | Not implemented; needs explicit consent and bounded encrypted transfers |
| Clipboard | Text/images on supported devices, opt-in | Not implemented; must avoid silent clipboard/credential capture |
| Media controls | Compatible phone players can be controlled | Not implemented; needs capability-scoped media-session commands |
| DND/volume | Available device controls; Android-version limitations | FocusBridge Study Mode is an app filter, not Android system DND |
| Instant Hotspot | Limited OEM/device support with Wi-Fi/Bluetooth prerequisites | Not implemented; ordinary app privileges cannot promise OEM system integrations |
| Desktop settings | Feature-level enable/disable | Rule text/app toggles exist; full desired/applied version and error reporting absent |
| Focused triage | General phone companion | Masked Peek, keyword/contact/app rules, Study lane, retention are FocusBridge's differentiators |
| Multi-device and user isolation | Multiple phones/PCs with account authorization | Current desktop sender is a single active socket; per-user/device routing must replace it before multi-device claims |

Feature sources:
[setup and device features](https://support.microsoft.com/en-us/windows/apps/phonelink/phone-link-requirements-and-setup),
[multiple devices](https://support.microsoft.com/en-US/Windows/Apps/phonelink/frequently-asked-questions-about-the-phone-link),
[notification actions](https://support.microsoft.com/en-US/Windows/Apps/view-and-manage-mobile-notifications-on-your-pc),
[SMS/MMS](https://support.microsoft.com/en-us/windows/apps/phonelink/send-and-receive-text-messages-from-your-pc),
[photos](https://support.microsoft.com/en-US/Windows/Apps/PhoneLink/setting-up-photos-in-the-phone-link),
[file and clipboard transfer](https://support.microsoft.com/en-us/windows/apps/phonelink/seamlessly-transfer-content-between-your-devices),
[media controls](https://support.microsoft.com/id-ID/Windows/Apps/PhoneLink/setting-up-notifications-in-the-phone-link),
[hotspot](https://support.microsoft.com/en-us/windows/experience/connectivity-networking/instant-hotspot),
[keyboard/device controls](https://support.microsoft.com/en-us/windows/apps/phonelink/keyboard-shortcuts-for-phone-link).

## Concrete source findings

- `desktop/src-tauri/src/sync/relay_client.rs` contains only `RelayConfig`, not a
  running desktop relay client. Android explicitly rejects non-local pairing.
- `request_device_reconnect` in `commands/pairing_cmd.rs` calls `send_to_phone`;
  no independent offline/wake channel exists. A saved IP is not remote reachability.
- `NotificationService.onNotificationRemoved` only updates local queue status;
  it does not send an acknowledged dismissal to desktop. It also marks a removed
  batch SENT without proof of desktop storage; this needs a delivery-policy fix.
- `commands/notification_cmd.rs` deletes desktop SQLite history, not the original
  Android notification. History deletion and notification dismissal need separate
  semantics and distinct UI labels.
- `set_study_mode` persists desktop state without sending it to Android.
  `RULES_UPDATE` must carry a versioned complete desired rule/mode snapshot.
- Desktop server now actively probes transport liveness. Android still relies on
  a 15-second retry supervisor and lacks a network-change callback.
- Whole-database encryption and replay-safe session/key management are being
  implemented/reviewed separately. Do not unblock relay based on existing AES
  encryption alone: the legacy relay AUTH key also derives the content key.

## Target architecture

### 1. Identity and consent

Keep guest LAN mode. For remote access, bind trusted device identities to a
verified account or a reviewed invitation/capability scheme. IP addresses and
display names are metadata, never identity. Each device has independent
revocable transport credentials, separate from device-only content keys.

New-PC pairing requires approval on the phone. Capabilities are explicit:
notifications, rules, remote reconnect, replies, files, media, and other controls
are separate grants. Account sign-in is not automatic authorization to read a
phone. Show linked devices and offer revocation on both clients.

### 2. Separate control and data paths

Both clients establish outbound WSS on port 443 to a public relay. LAN/hotspot
remains the preferred data path and must work without the Internet. A private DNS
name does not create connectivity through NAT or client isolation.

Control path: expiring reconnect requests, device capabilities, and authenticated
presence. Optional FCM wake notification can ask the user to reopen/approve a
connection; it must not contain messages, keys, or reusable credentials.

Data path: end-to-end encrypted notifications, rules, actions, inventories, and
ACKs. The relay only routes bounded opaque ciphertext. A relay connection alone
must never cause the UI to claim the phone is connected.

Use an established authenticated handshake with fresh session keys and ordered
transport counters. Reject replay, reflection, wrong-peer frames, truncation,
tampering, and downgrade. Reconnect starts a new cryptographic session; pending
application records retain stable IDs and are re-encrypted for that session.
Do not cache old session ciphertext as the only durable delivery copy.

### 3. Connection state and command delivery

One shared state machine owns per-device transport, reason, last verified peer
activity, handshake generation, retry deadline, and applied rules version. UI
components observe it; they do not independently infer connection from different
heartbeat thresholds or stale database flags.

- Pause notifications: retain only the explicitly approved reconnect/control
  path; no notification bodies flow while paused.
- Disconnect completely: stop all app control/data paths. Reopening/approving on
  the phone is required; remote silent restart contradicts a full cutoff.
- Revoke device: invalidate its capabilities/keys; re-pairing requires approval.
- Reconnect: request ID, expiry, requesting PC identity, approval/denial result,
  then authenticated handshake and queue reconciliation. Show "Waiting for phone
  approval", "Offline", or "Expired", not a false connected state.

Every mutation uses a stable command ID and receipt/applied ACK. Keep pending
commands durable and bounded; retry safely, discard expired actions, and never
turn a retry into a duplicate reply or deletion. Transport failover must elect
one active session rather than deliver everything twice over LAN and relay.

### 4. Platform and cost limits

Free-first candidate: Workers Free with SQLite-backed Durable Objects,
hibernating WebSockets, hard quota limits, and a workers.dev test hostname.
The owner supplied the `focusbridge.workers.dev` account subdomain on 2026-09-05;
deployment is not live. Follow `free-relay-options.md`; no billing or
auto-charging trial is authorized. An account subdomain alone is not a relay.

FCM is not a way to defeat Android force-stop, a powered-off phone, denied
permissions, or a missing Internet connection. High-priority push must be used
for user-visible notifications, not a hidden persistent keepalive.
[Android Doze guidance](https://developer.android.com/training/monitoring-device-state/doze-standby).

Remote access cannot bypass an administrator who blocks the service. Never
silently disable TLS, install interception CAs, or turn off a user's firewall.

## Ordered acceptance gates

1. Regression-test LAN disconnect/reconnect, background notification delivery,
   batching, duplicate suppression, inventory replacement, and mode/rule ACKs.
2. Isolated encrypted-storage migration fixtures: wrong key, missing key, crash
   at each migration boundary, locked UI with working background sync, and no
   readable plaintext database after successful migration.
3. Shared crypto vectors and real cross-language handshakes; tampering, replay,
   reconnect, wrong-device identity, downgrade, and key-revocation tests.
4. Free relay implementation with authenticated ownership, bounded queues,
   expiry, quota failure, backpressure, restart recovery, and no private logs.
5. Real mobile-data-to-PC-Wi-Fi test; approval from the PC, offline recipient,
   app background/Doze, route change, relay outage, and LAN fallback.
6. Add notification actions/media/file conveniences only through tested,
   consent-scoped commands. Show unsupported features honestly.
7. Signed release artifacts, clean-PC install, broader Android/OEM matrix,
   privacy review, and an independent security review before public release.

No commit or release is authorized merely because a subset of tests passes.
