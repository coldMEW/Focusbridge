# Phone Link comparison and FocusBridge connection architecture

Reviewed: 2026-09-05. This is a source-backed comparison and implementation
contract, not a claim that the proposed functionality is already available.
Microsoft's public support documentation describes product behavior, not its
complete private server implementation or cryptographic protocol.

## What "anywhere" actually means

Reviewed again 2026-09-06 against Microsoft's current documentation, because
this claim drove the whole relay design and it is worth being precise about.

Phone Link does **not** hold a connection open from anywhere by magic. Its own
troubleshooting page states: "To ensure the fastest, most reliable connection,
your Android device and PC must be connected to the same trusted Wi-Fi network."
Cross-network use is an explicit opt-in toggle inside Link to Windows —
*Settings > Sync over mobile data > On* — which Microsoft recommends leaving off
to avoid data charges.

Microsoft documents the same obstacles FocusBridge faces, in the same words:

- **Router AP isolation.** "If this Wireless Isolation (or AP Isolation) is
  enabled, then all devices connected to the Wi-Fi network will be blocked from
  communicating." This is why same-Wi-Fi alone is not sufficient, for either app.
- **Battery optimisation.** Link to Windows must be exempted or connections are
  interrupted — the same requirement FocusBridge puts in its setup checklist.
- **Background execution.** Phone Link must be allowed to run in the background
  on the PC.

Microsoft never documents a relay, but a mobile-data sync toggle cannot work
without a rendezvous point: two devices on different carrier networks have no
route to each other. So the architecture is necessarily the same shape as the
one FocusBridge now uses.

**FocusBridge matches this model, and differs in one way.** Phone Link makes
cross-network sync a setting the user must find and enable. FocusBridge tries
every local address first and falls back to the relay automatically, so there is
nothing to turn on for it to keep working when you leave the house. The relay
itself is opt-in, because it needs an account; the fallback is not.

Sources: [connectivity troubleshooting](https://support.microsoft.com/en-us/windows/apps/phonelink/troubleshooting-the-phone-link),
[requirements and setup](https://support.microsoft.com/en-us/windows/apps/phonelink/phone-link-requirements-and-setup),
[frequently asked questions](https://support.microsoft.com/en-us/topic/frequently-asked-questions-about-the-phone-link-7ccef61d-7bbd-2b26-77ec-76e9a358c25d),
[calls](https://support.microsoft.com/en-us/windows/apps/phonelink/setting-up-calls-in-the-phone-link),
[Apps troubleshooting](https://support.microsoft.com/en-us/windows/apps/phonelink/troubleshoot-apps-in-the-phone-link).

## Feature comparison

"Present" means implemented and exercised on the confirmed test hardware — a
Pixel 7 and a Windows 11 desktop — not certified on every device.

| Capability | Phone Link | FocusBridge |
| --- | --- | --- |
| Same-network sync | Recommended path | **Present.** Preferred automatically; needs no account and no Internet |
| Different-network sync | Opt-in "sync over mobile data" | **Present.** Verified phone-on-cellular to PC-on-Wi-Fi; falls back without being asked |
| Reconnect initiated from the PC | Account-linked device workflow | **Present.** The phone keeps a control-only presence at the relay, so the PC can ask from any network and the phone prompts before letting it in |
| Pairing | Account-linked onboarding, QR | **Present.** QR or manual, with a confirmation showing the address, whether the PC will be reachable off-LAN, and a certificate code to compare |
| Saved devices | Account device list | **Present.** Stable install identity, transport shown honestly as local or relay |
| Notifications | Capture, display, app selection | **Present**, with delivery acknowledgements and a retry queue |
| Notification filtering | App selection only | **Better.** Per-app mute/priority/study-safe, priority and blocked keywords, favourite contacts — enforced on the phone, so filtered notifications are never transmitted |
| Privacy of content in transit | Not documented | **Better.** Sealed end to end; the relay routes ciphertext it holds no key for |
| Encryption at rest | Not documented | **Present.** SQLCipher on both ends, keys in DPAPI and the Android Keystore |
| Notification actions and replies | Supported; dismissals propagate | **Missing.** Dismissal is one-way and desktop deletion only clears local history |
| SMS/MMS | Read and send | **Missing**, and out of scope: mirroring a message notification is not SMS access |
| Calling | Bluetooth calling, recent calls | **Missing**, out of scope |
| Photos | Recent photos on the PC | **Missing**, out of scope |
| App streaming / phone screen | Supported devices only | **Missing**, out of scope |
| Clipboard | Opt-in, supported devices | **Missing.** Deliberate: silent clipboard capture would read passwords |
| File transfer | Supported | **Missing**, out of scope |
| Media controls | Compatible players | **Missing.** The most defensible future addition of these |
| Instant hotspot | Limited OEM support | **Missing.** Ordinary app privileges cannot deliver this |
| Do not disturb / volume | Device controls | Study Mode is an app filter, not system DND |
| Multiple phones and PCs | Account authorization | **Partial.** One active phone per desktop; both ends can now refuse an automatic reconnect, but per-device routing is still needed before claiming multi-device |

## Where the remaining difference actually matters

Most of the missing rows are missing on purpose. FocusBridge is a notification
triage tool, not a phone mirror, and half-building SMS or screen streaming would
make it worse at the thing it is for.

One row is a genuine gap rather than a scope decision:

**Notification actions.** Phone Link lets you reply to a message and dismiss a
notification from the PC, and the dismissal reaches the phone. FocusBridge can
show a notification and delete it from desktop history, but the phone never
hears about it, so the same alert is still waiting on the phone afterwards. For
an app whose entire purpose is not picking the phone up, that is the wrong
ending: triage on the desktop should be able to finish the job.

This needs Android `RemoteInput` and notification action handling, a versioned
command with a stable identifier so a retry cannot send a reply twice, and a
clear separation in the UI between "clear from this list" and "dismiss on my
phone" — which today are the same button and mean the weaker of the two.

## Source findings, and what became of them

Recorded on 2026-09-05, resolved on 2026-09-06 unless marked otherwise.

- **Fixed.** `sync/relay_client.rs` held only a `RelayConfig` struct, and Android
  rejected any non-local pairing outright. Both ends now dial the deployed relay
  and run a device-only session over it.
- **Fixed.** `request_device_reconnect` called `send_to_phone`, so it could only
  reach a phone that was already connected. It now joins the relay and waits,
  and the phone keeps a control-only presence there so it can be asked at all.
- **Fixed.** The desktop encrypted everything after `AUTH_OK`, and the relay
  transport dispatched without unwrapping that layer, so every reply was
  discarded — including the heartbeat response, which ended each session after
  about three minutes.
- **Open.** `NotificationService.onNotificationRemoved` still only updates local
  queue status rather than sending an acknowledged dismissal, and still marks a
  removed batch SENT without proof the desktop stored it.
- **Open.** `commands/notification_cmd.rs` deletes desktop history, not the
  notification on the phone. These need separate semantics and distinct labels;
  see the notification-actions gap above.
- **Open.** `set_study_mode` persists desktop state without sending it to
  Android. `RULES_UPDATE` should carry a versioned complete desired snapshot.
- **Open.** Android has no network-change callback and still relies on a
  15-second retry supervisor, so a Wi-Fi to cellular switch is noticed late.
- **Fixed.** Whole-database encryption is active on both platforms, and the
  replay-safe session is the shared Noise engine rather than the old
  pairing-key envelope. The relay is no longer blocked.

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
