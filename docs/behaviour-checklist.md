# Behaviour checklist

Every feature and every fixed bug, with the behaviour it is supposed to have and
the test that holds it there. **Read this list before committing and confirm each
row still holds.** Fixes on this project have repeatedly undone one another —
making a manual disconnect stick broke reconnecting a known phone, and restoring
that re-broke silent reconnection — and each of those reached the user before it
reached a test.

Add a row when a bug is fixed or a feature is added. If two rows conflict, that is
a design question to settle out loud, not to resolve quietly in code.

## The rules that outrank everything

| # | Rule | Where it is decided |
|---|------|---------------------|
| R1 | The phone's "let a known PC reconnect" toggle is absolute. On: a PC that has connected before reconnects with no notification and no prompt. Off: it asks every time. | `SyncEngine.autoReconnectEnabled()` feeding `requireApproval` |
| R2 | Disconnect means disconnect. After Disconnect on the desktop, that PC does not reach for the phone again by itself — not for a preference, not because the pairing screen is on display. Only picking the phone under previous connections, or asking for a fresh code, resumes it. | `AppState::is_paused` / `resume` |

R1 and R2 meet when a disconnected phone is asked for by name. **R1 governs there**,
because the PC asked deliberately; what R2 forbids is the PC reaching out unasked.

## Pairing

| Behaviour | Verified by |
|---|---|
| The QR is compact enough to scan: the link stays under 425 characters, roughly 73 modules | `qr::tests::the_scanned_link_stays_short_enough_to_read_from_a_distance` |
| Desktop encoder and phone decoder agree field for field | `qr::tests::the_compact_form_round_trips_every_field`, `CompactPairingPayloadTest.readsEveryFieldTheDesktopWrote` (decodes bytes the Rust encoder produced) |
| A payload the compact form cannot carry still pairs, as the older JSON link | `qr::tests::a_payload_the_compact_form_cannot_carry_falls_back_to_the_json_link` |
| A pasted JSON payload still pairs | `CompactPairingPayloadTest.aPastedJsonPayloadStillWorks` |
| A truncated or padded code is refused, never half-read | `qr::tests::a_truncated_or_padded_code_is_refused_rather_than_half_read`, `CompactPairingPayloadTest.aDamagedCodeIsRefusedRatherThanHalfRead` |
| The code is big enough on screen to scan (fills the side panel; click to fill the screen) | Manual: measure pixels per module on a screenshot; the side panel was 224px, under 3px a module |
| A deep link cannot pair the phone without a confirmation showing address, cross-network and security code | Android guardrail test |

## Connecting

| Behaviour | Verified by |
|---|---|
| A phone with no pinned key on this PC enrolls; a pinned phone authenticates and is never silently replaced | `relay_client::tests::a_phone_with_no_pinned_key_enrolls_and_a_pinned_one_authenticates` |
| A rejected handshake beside a live pairing code enrolls on the next attempt; a timeout does not | `relay_client::tests::only_a_rejected_handshake_beside_a_live_code_opens_enrollment` |
| Cross-network sync works with the phone on cellular and the PC on Wi-Fi | Real device: relay socket → secure session ready → phone authenticated on 127.0.0.1 |
| Reconnecting a known phone works on any network | Real device, over cellular; `relay connection requested reason="the user asked to reconnect a saved phone"` |
| Rendering the pairing panel does not make this PC reachable; only pressing the button does | `for_pairing` is false for the sidebar panel |
| The desktop does not attach to the last phone when automatic reconnection is off, **on any transport** — local network included, not only the relay | `ws_server::attach_tests`. This shipped broken twice: the check asked `peer.ip().is_loopback()`, so the setting worked over the relay and did nothing over the LAN, and the desktop reattached on every launch |
| A phone turned away by that setting is not retried in a loop | `state.take_known_phone_refusal()` sends the relay client to idle. It used to reconnect straight back into the same refusal every few seconds for as long as the pairing screen was on display |
| The one-time allowance is spent only when it is actually needed | `ws_server::attach_tests::the_allowance_is_not_spent_unless_it_is_needed` |
| A pairing session survives a desktop restart between scanning and connecting | persisted as `pairing.session.v1` |
| The local transport probe is answered over the relay bridge, so sessions do not die after two minutes | `Message::Ping` → `Pong` in the bridge |

## Disconnecting

| Behaviour | Verified by |
|---|---|
| A manual disconnect is not lifted by the reconnection switch: nothing syncs unasked | `SyncEngineTest.aManualDisconnectIsNotLiftedByTheReconnectionSwitch` |
| A disconnected phone with the toggle **on** reconnects silently when a PC asks for it | `SyncEngineTest.aDisconnectedPhoneWithTheSwitchOnReconnectsWithoutAsking` |
| A disconnected phone with the toggle **off** waits, and asks before anything is decrypted | `SyncEngineTest.aDisconnectedPhoneWithTheSwitchOffWaitsToBeAsked` |
| A LAN-only pairing goes fully offline on disconnect, because there is no way to be asked | `SyncEngineTest` (no relay case) |
| One prompt per arrival, not one per relay event | `alreadyAsking` dedupe in `WebSocketClient` |
| The phone shows disconnected when the desktop disconnects | heartbeat + `UNPAIR` handling |
| An authenticated session ends the manual disconnect, so the two states cannot disagree | `WebSocketClient` clears it on `AUTH_OK` |
| A connected phone sends notifications even if a disconnect flag was never cleared | `SyncEngineTest.aConnectedPhoneSendsEvenIfADisconnectWasNeverCleared` |
| A genuinely disconnected phone holds notifications as pending rather than dropping them | `SyncEngineTest.aDisconnectedPhoneHoldsNotificationsInsteadOfSending` |

## Pairing again after a disconnect

| Behaviour | Verified by |
|---|---|
| Asking for a pairing code resumes this PC even when the previous code is still valid | `pairing_cmd::pairing_request_tests`. Resuming was tied to minting a *new* session, so scanning after a disconnect resumed nothing and a phone on mobile data sat at "connecting" |
| The pairing code is **always** shown, with no extra step, and a shown code always works | `PairingQR.test.tsx`. A disconnected PC still waits where a phone scanning that code can find it; being findable is not the same as accepting |
| Showing a code never lifts a disconnect; a phone arriving with that code does | `pairing_cmd::pairing_request_tests::showing_a_code_never_lifts_a_disconnect`, and `ws_server` resumes on `just_scanned`. The panel appears by itself whenever nothing is connected, so anything it does on its own undoes the user's disconnect |
| Scanning is consent: the phone does not ask permission for a pairing just made | `SyncEngineTest.scanningACodeConnectsWithoutAskingEvenWithTheSwitchOff` |
| A pairing the desktop cannot recognise stops retrying and says to scan again | AUTH_FAILED carries `unknown_pairing`; the phone shows it and stops dialing, instead of retrying every 15s forever on mobile data |
| On mobile data the phone does not wait on LAN addresses that cannot answer | `SyncEngineTest.onMobileDataTheLocalAddressesAreNotWaitedOn`; this was 4s per saved address before the relay was even tried |

## Notifications

| Behaviour | Verified by |
|---|---|
| A notification captured on the phone reaches the desktop | Real device: `captured 1 from <app>` on the phone, `notification received app=<app> stored=true` on the desktop, ~200ms apart |
| Every drop on the path is visible, so "nothing arrives on the PC" can be answered rather than guessed | `FocusBridgeSync` log lines on the phone, `notification received` on the desktop; no message content is logged |
| The minified release build keeps the capture path working | Release APK on a device: capture logged, no serialization or reflection failure |

## Security

| Behaviour | Verified by |
|---|---|
| Conversations are encrypted end to end, and the relay routes opaque frames | `Noise_XXpsk3_25519_ChaChaPoly_SHA256`; relay stores capability hashes only |
| The database is unreadable without the key, WAL and sidecars included | `encrypted_database` suite, against an independent SQLite build |
| The unauthenticated connection ceiling holds | `MAX_UNAUTHENTICATED_CONNECTIONS`; measured 179/200 refused |
| Relay pair provisioning requires a verified Firebase identity | relay worker suite |
| A TLS provider is chosen before any connection is made | `tls_provider_tests::a_tls_provider_is_chosen_before_any_connection_is_made`; rustls 0.23 turns a missing choice into a runtime panic that no other test catches |
| The webview runs under a Content Security Policy, and holds no permission it does not use | `tauri.conf.json` security.csp; capabilities list |
| Release builds are signed with a private key kept outside the repository | `:app:signingReport` shows the FocusBridge key for the release variant |
| Dependency advisories are checked | `pnpm audit --prod` clean; `cargo audit` — every remaining crate (`quick-xml`, `quinn-proto`, the GTK bindings) is absent from `cargo tree --target x86_64-pc-windows-msvc`, so none of it is in the shipped binary |
| The stored inbox is not readable while the vault is locked, by any route | `notification_cmd::tests::the_inbox_is_refused_while_locked_and_served_once_open`; the notification event is not emitted to the interface either |
| An install is verified against the payload of the installer that was run | `install-desktop.ps1` extracts the .msi and compares hashes |
| The relay rejects unauthenticated sockets before the upgrade, and bad roles and query strings outright | live probes: 401 / 404 / 400 |
| No message content is shown before the local vault is unlocked; a launch starts locked, and the idle timeout and signing out close it again | `state::vault_lock_tests`; the desktop notification is gated on `vault_is_unlocked`. The lock was interface-only, so messages appeared in full on screen while the PIN was still being asked for |
