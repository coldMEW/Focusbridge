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
| The desktop does not auto-connect to the last phone when automatic reconnection is off | AUTH gate in `ws_server`: a saved key needs an explicit allowance |
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

## Security

| Behaviour | Verified by |
|---|---|
| Conversations are encrypted end to end, and the relay routes opaque frames | `Noise_XXpsk3_25519_ChaChaPoly_SHA256`; relay stores capability hashes only |
| The database is unreadable without the key, WAL and sidecars included | `encrypted_database` suite, against an independent SQLite build |
| The unauthenticated connection ceiling holds | `MAX_UNAUTHENTICATED_CONNECTIONS`; measured 179/200 refused |
| Relay pair provisioning requires a verified Firebase identity | relay worker suite |
