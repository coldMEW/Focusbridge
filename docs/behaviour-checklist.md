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
| The desktop does not attach to the last phone when automatic reconnection is off, **on any transport** — local network included, not only the relay | `attach::attach_tests` (in `focusbridge-core`). This shipped broken twice: the check asked `peer.ip().is_loopback()`, so the setting worked over the relay and did nothing over the LAN, and the desktop reattached on every launch |
| A phone turned away by that setting is not retried in a loop | `state.take_known_phone_refusal()` sends the relay client to idle. It used to reconnect straight back into the same refusal every few seconds for as long as the pairing screen was on display |
| The one-time allowance is spent only when it is actually needed | `attach::attach_tests::the_allowance_is_not_spent_unless_it_is_needed` |
| A pairing session survives a desktop restart between scanning and connecting | persisted as `pairing.session.v1` |
| The local transport probe is answered over the relay bridge, so sessions do not die after two minutes | `Message::Ping` → `Pong` in the bridge |
| A late pong does not end a healthy session. The probe waits 20s and is then **retried**; only 90s of total silence ends the session, inside the 180s the desktop advertises in `AUTH_OK` | `heartbeat::tests::a_late_pong_does_not_end_the_session`. It used to end on the first probe unanswered for six seconds while the phone had been told it had a hundred and eighty, so a session lasted 30s, or 2min, or three hours, entirely according to when the handset's radio was slow |
| An **application frame** from the phone counts as proof of life, not the transport pong alone, so a phone busy sending notifications is never dropped for a queued pong | `heartbeat::tests::an_application_frame_counts_as_proof_of_life`; `mark_alive` on inbound text frames in `ws_server` and `socket_io` |
| A **pong** only counts if it matches the probe still outstanding and unexpired. A stray or long-late pong is not proof of life | `heartbeat::tests::a_pong_nobody_asked_for_proves_nothing`, `connection_state::unrelated_or_late_pongs_do_not_extend_connection_lifetime`. The first cut of the timeout fix let any frame at all extend the session, which would have made the challenge measure nothing |
| Probing never buys the session time; only the phone can | `connection_state::expired_idle_heartbeat_cannot_be_revived_by_a_delayed_probe`. Enforced structurally now — the deadline is measured from when the phone was last heard from, which no probe touches — rather than by refusing to probe past the deadline |
| Application work longer than one probe timeout still services control frames | `connection_state::slow_batch_longer_than_response_timeout_keeps_servicing_control_frames`, on a paused clock so the real 15s/20s timers are exercised without a 45s test |
| Waking to probe always has something to send, so the connection loop cannot spin | `heartbeat::tests::waking_never_spins_the_caller` |
| The interface never reports a disconnection the backend did not make | `connectionHealth.test.ts` — `STALE_HEARTBEAT_MS` (100s) sits outside the backend's 90s silence budget. At 12s it contradicted a live session |
| **Feature 1 (the user's):** "Reconnect to the last phone automatically" decides one thing only — may this PC take a saved phone that arrives unasked, at startup or any other time. Off, it does not reach out on its own | `attach::attach_tests::the_setting_off_refuses_a_saved_phone_that_arrives_unasked`, `::the_setting_on_lets_a_saved_phone_straight_in`, `::a_saved_phone_attaches_when_the_user_asked_for_it_by_name` |
| **Feature 2 (the backend's):** a connection the user already made is kept alive across a dropped transport — relay socket replaced, Wi-Fi handover, packet loss — without asking again. Not a setting, and never one | `attach::attach_tests::resuming_works_with_the_setting_off`, `::resuming_does_not_need_the_setting_on_or_a_code_on_screen`, `state::connected_phone_tests::the_phone_that_connected_may_come_back`. **These two must never be folded back into one flag.** They were, and feature 1 silently did feature 2's job with a single-use allowance: spent on the first attach, so the phone was refused the moment anything under it flinched. A stable connection lasted 30s, or 2min, or three hours — not a timer, but whenever the first hiccup landed. Seen live as `refused a phone ... automatic=false paused=false` |
| Feature 2 is not a way around feature 1: it covers one phone, is never carried across a restart, and does not survive a manual disconnect | `attach::attach_tests::only_the_connected_phone_resumes_a_different_one_still_meets_the_setting`, `::a_disconnect_outranks_resuming`, `state::connected_phone_tests::only_that_phone_may_come_back`, `::a_launch_knows_no_connected_phone`, `::a_manual_disconnect_ends_the_standing`. Held in memory only; cleared in `mark_manual_disconnect` |
| Resuming never spends the one-time allowance, which belongs to the next phone asked for by name | `attach::attach_tests::resuming_never_spends_the_allowance` |
| **A refusal stays quiet on both ends.** After turning a saved phone away (automatic reconnection off), this PC stays on its relay socket instead of leaving and rejoining, and the phone, told it was turned away, waits at the relay to be asked instead of re-dialing. Nothing is knocked off; a code on screen keeps working; a new phone scanning it still gets in. Only the user picking the phone, or turning automatic reconnection on, makes this PC join again | `state::connected_phone_tests::only_a_deliberate_request_makes_a_waiting_pc_rejoin`; `SyncEngineTest.aPhoneTurnedAwayWaitsAtTheRelayInsteadOfDialingBack`, `.aTurnedAwayPhoneWithNoRelayKeepsDialing`, `.aTurnedAwayPhoneHoldsNotificationsInsteadOfDialing` (a new notification used to make the waiting phone dial out and close its relay socket). Real device: after a restart, one refusal on each transport and then 20 minutes with no refusal, rejoin or kick. Settled with the user on 2026-09-23: the pairing panel refreshing itself (every 15s and on window focus) called `request_relay_connection`, which lifted the parking below, and every rejoin retired the pair and closed the phone's socket -- a disconnect/reconnect every ten to twenty seconds for as long as the phone had not been picked, seen in the log 06:39-06:53 |
| A refused phone parks this PC until the user asks for something, rather than dialing the relay straight back into the same refusal | `state::connected_phone_tests::a_refusal_parks_this_pc_until_the_user_asks`, `::the_safety_net_timeout_is_not_the_user_asking`, `::a_phone_getting_in_clears_the_parking`. Rejoining retires the pair at the relay and the relay closes **both** sockets, so the loop was kicking the phone off every 35s — 30s of safety-net wait plus 5s of backoff — for as long as a pairing code was on screen. Measured at 18:34:51 → 18:35:26 in a live log, 50 cycles in 34 minutes |
| Every test this checklist names actually runs | `[lib] test = false` is gone from `desktop/src-tauri/Cargo.toml`, so the 22 unit tests in `src/` execute on every `cargo test`. They were compiled by nothing and run by nothing, while this checklist named several of them as the guarantee — `pairing_cmd::pairing_request_tests`, `notification_cmd::tests`, `relay_client::tests` and `tls_provider_tests` among them. That is why the connect/disconnect rules regressed three times without a single test going red. The attach rule also moved to `focusbridge-core` (`desktop/core/src/attach.rs`), where it is pure logic with no Tauri context at all |
| A burst of application frames larger than the pending queue is **back-pressured, not fatal**. Reading stops; the frames wait in the socket; nothing is lost | `a_burst_larger_than_the_queue_is_back_pressured_rather_than_fatal`, `a_full_queue_reports_itself_full_before_a_push_can_fail`. **This was the disconnection.** A phone that reconnects flushes the notifications it was holding, the whole backlog lands while one of them is being written to the database, and past `MAX_PENDING_MESSAGES` the queue refused the next frame and killed the session with "pending websocket work limit exceeded". The acknowledgements were therefore never sent, so the phone kept the same notifications pending and flushed the identical burst on the next connection — a loop that killed every session about a second after it authenticated. The memory bound is unchanged; only the response to reaching it is |
| The relay says which side ended a session, and how big a record was | `relay_client` logs the WebSocket close code (the relay retires a pair with 1012/4003/4008/1009, while a phone that simply leaves sends no close at all) and logs record size and frame count, sizes only and never content. "the phone disconnected" was otherwise the only thing visible, and it does not say why |
| A PC waiting at the relay stays there: it sends a keepalive while waiting, not only during a session, and a socket that held for minutes does not grow the backoff | `relay_client::attempt` pings every `KEEPALIVE` and counts any inbound frame, pongs included, as proof of life. It used to hear nothing for 150s, call that a dead socket and redial -- dozens of times a day in the log as `relay socket idle timeout`. Each redial closed the old socket, and the relay's `webSocketClose` **retires the pair and closes the phone's socket too**, so a phone waiting at the relay was kicked every few minutes. The doubling backoff also left the PC absent for up to two minutes when the phone did need the relay |
| **A relay blip is not a disconnection.** When a connection the user made drops without anyone asking, the desktop shows "Reconnecting..." for up to 90s, keeps the inbox on screen, and raises no popup. Only a loss that outlasts the grace is announced ("disconnected"), and only a connection after an announced loss says "connected". A manual disconnect is never treated as a reconnect | `state::connected_phone_tests::a_blip_inside_the_grace_is_never_announced`, `::a_loss_that_outlasts_the_grace_is_announced_once`, `::a_newer_loss_replaces_an_older_one`, `::a_manual_disconnect_is_not_a_reconnect`; `connectionHealth.test.ts` (reconnecting while the backend says so, not past the grace). The relay's host (Cloudflare) cuts long-lived sockets every 20-35 minutes, sometimes in bursts: on 2026-09-23 the desktop's relay socket (`read relay frame`), the phone's (`EOFException`, on LTE) and an unrelated `wrangler tail` session from the same PC all dropped in the same second, with no close frame and no event in the relay code. It cannot be prevented from here; before this, every one of them raised two popups, emptied the inbox and put the pairing screen up. `notified_connected` was per socket, so each replacement socket announced itself afresh |
| Session-end reasons carry the whole error chain, not just the outermost context | `{error:#}` in `relay_client` and `ws_server`. "read relay frame" alone hid the cause of every relay drop |
| The phone says why its socket ended | `WebSocketClient.logSocketEnd`: transport, close code or exception class and message, never content. Before this the desktop could only log "the phone disconnected", and the phone logged nothing at all |
| The desktop writes a log to disk, and says why a session ended | `install_logging` writes `%APPDATA%\com.focusbridge.desktop\logsocusbridge.log` (one rotation at 8MB), defaulting to `info` rather than to silence; `state::clear_phone_sender_if_current_with_reason` logs the reason. This is a windowed process with no console and no `RUST_LOG`, so it produced no diagnostics at all: a drop that happens once every few hours had no answer anywhere on the machine. No call site logs message content, capabilities or key material |

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
| Disconnecting retires the code on screen, so the phone just let go of cannot present it | `state::vault_lock_tests::disconnecting_retires_the_pairing_code_on_screen`. A phone keeps its pairing key and the session lives five minutes, so it still held the live key and was read as having just scanned it — the disconnect lasted about seven seconds |
| The saved device's certificate fingerprint is never overwritten with a blank | `ws_server` uses this PC's own certificate, not the pairing session's copy, which is cleared on disconnect |
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
| The inbox is empty until a phone is attached, and the stored history appears **on connect** | `App.tsx` loads `list_notifications` on the transition to `CONNECTED` and clears the view otherwise. It used to load once on mount — before the vault is unlocked, so the backend refused it, the failure was only logged, and nothing asked again: yesterday's notifications never came back |
| History loading never overwrites a notification that arrived while it was loading | `notificationStore.mergeHistory` folds stored rows in under the live ones by id |
| **Capture heals itself.** If Android stops delivering notifications to the listener while access is still granted, the app notices within a minute, rebinds the listener, and captures from the shade whatever it missed | `ListenerWatchdogTest`; `NotificationService.inspect`, checked every 60s by `SyncForegroundService.watchListener`. Real device: `adb shell cmd notification post` produced no `captured` line until the listener was rebound. **This was "stops syncing after ~150 messages".** On a Pixel 7 the listener stayed registered and granted but received no callbacks from 2026-09-16 on; the phone showed Connected and the desktop received nothing for a week. Toggling notification access by hand fixed it instantly, which is what the watchdog now does: `requestRebind`, then a component reset if that is not enough |
| **One message, one row.** An app updating a notification (Spotify, Pocket Casts: play, pause, progress) does not produce a new message unless what it says has changed; a chat notification repeating the conversation (WhatsApp) sends only the message that is new | `LiveNotificationsTest` (a re-post under the same key with the same content is an update; the same words after a dismissal are new; the shade is re-read on connect so a restart does not re-capture what is showing); `NotificationIdentityTest.aChatMessageKeepsItsIdWhenNewerMessagesPushItDownTheConversation`; `NotificationService.capture` skips ids already stored. The id used to include the message's position in the conversation, which WhatsApp shifts with every new message, and plain notifications were keyed by post time, which changes on every update |
| A copy that still reaches the desktop -- an older phone build, the same message under another id -- is acknowledged but not stored, shown or popped up again | `app_inventory::the_same_message_under_another_id_is_recognised_as_a_copy`; `store::equivalent_notification` matches app, sender, non-empty text and send time. Masked (empty) texts are never merged |
| A long message can be read in full, on both apps, the way the shade expands it: two lines by default, "Show full message" / "Show less" when there is more | Desktop `NotificationCard.test.tsx` (long message toggles, short has no toggle, a masked message is not expanded around the peek). Android `MobileNotificationRow` shows the toggle when the text overflows two lines. Inbox-style notifications now carry their expanded lines, not only the one-line preview (`DefaultParser.inboxLines`) |
| After a reconnect the inbox shows up to 500 stored rows, not 150 | `App.tsx` `HISTORY_LIMIT`, matching the backend clamp in `store::list_notifications` |
| Desktop popups can be turned off without affecting sync or the inbox | `settings_cmd::set_desktop_notifications` → `AppState::desktop_notifications_enabled`, checked at all three `desktop_notifications::show_*` call sites. Default on; the stored preference is re-read at startup |
| Turning popups off never suppresses storage or the in-app inbox | the gate sits at the popup call site only; `store_notification` and the `focusbridge://notification` event are untouched |

## The desktop window

| Behaviour | Verified by |
|---|---|
| Launching FocusBridge while it is already running (Start search, a shortcut, the installer's "launch") brings the running window forward -- unminimized, shown from the tray, focused -- and starts nothing else | `tauri-plugin-single-instance`, registered first in `lib.rs`, calling `window::reveal_main_window`. A second launch used to fail on the database lock during setup and exit with nothing on screen |
| Left-clicking the tray icon opens the window; right-clicking shows the menu | `tray::menu::install`, `show_menu_on_left_click(false)`. Left click used to open the menu |
| Close still asks "run in tray or quit" | `on_window_event` `CloseRequested` in `lib.rs`, unchanged |

## Security

| Behaviour | Verified by |
|---|---|
| Conversations are encrypted end to end, and the relay routes opaque frames | `Noise_XXpsk3_25519_ChaChaPoly_SHA256`; relay stores capability hashes only |
| The database is unreadable without the key, WAL and sidecars included | `encrypted_database` suite, against an independent SQLite build. `FOCUSBRIDGE_SQLITE3_TEST_BIN` must point at an ordinary (non-SQLCipher) reader, or `independent_sqlite_cli_cannot_read_encrypted_database` fails with "ordinary SQLite CLI required" — which reads as a broken build rather than a missing tool. On a machine with no `sqlite3.exe`, use `desktop/scripts/ordinary-sqlite3.cmd`, whose CPython-linked SQLite has no cipher support at all |
| The unauthenticated connection ceiling holds | `MAX_UNAUTHENTICATED_CONNECTIONS`; measured 179/200 refused |
| Relay pair provisioning requires a verified Firebase identity | relay worker suite |
| A TLS provider is chosen before any connection is made | `tls_provider_tests::a_tls_provider_is_chosen_before_any_connection_is_made`; rustls 0.23 turns a missing choice into a runtime panic that no other test catches |
| The inbox cannot be exported from the window: no right-click menu (so no "Save as" or "Print"), no Ctrl+S / Ctrl+P / Ctrl+U, no developer tools in a release build, no dragging content out. Copy, paste and text fields work as before | `window::lock_down_webview` switches WebView2's default context menus and browser accelerator keys off; `pageLockdown.test.ts` covers the in-page half. Right-click used to offer "Save as", which wrote every message on screen to an HTML file |
| The webview runs under a Content Security Policy, and holds no permission it does not use | `tauri.conf.json` security.csp; capabilities list |
| Release builds are signed with a private key kept outside the repository | `:app:signingReport` shows the FocusBridge key for the release variant |
| Dependency advisories are checked | `pnpm audit --prod` clean; `cargo audit` — every remaining crate (`quick-xml`, `quinn-proto`, the GTK bindings) is absent from `cargo tree --target x86_64-pc-windows-msvc`, so none of it is in the shipped binary |
| The stored inbox is not readable while the vault is locked, by any route | `notification_cmd::tests::the_inbox_is_refused_while_locked_and_served_once_open`; the notification event is not emitted to the interface either |
| An install is verified against the payload of the installer that was run | `install-desktop.ps1` extracts the .msi and compares hashes. Pass `-ExpectContains <string>` to also prove a specific change shipped — use a literal that survives into the release binary, never one from a `#[cfg(test)]` block |
| Installing a new build over an old one keeps this PC's identity and its inbox: the certificate and the encrypted database are reused, not regenerated | After `install-desktop.ps1`, `%APPDATA%\com.focusbridge.desktop` still holds the original `desktop-cert.pem`/`desktop-key.pem` and `focusbridge.db`. A regenerated certificate would silently break every saved pairing, and the phones would be turned away with no way to see why |
| The relay rejects unauthenticated sockets before the upgrade, and bad roles and query strings outright | live probes: 401 / 404 / 400 |
| No message content is shown before the local vault is unlocked; a launch starts locked, and the idle timeout and signing out close it again | `state::vault_lock_tests`; the desktop notification is gated on `vault_is_unlocked`. The lock was interface-only, so messages appeared in full on screen while the PIN was still being asked for |
