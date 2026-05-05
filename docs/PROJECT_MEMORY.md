# FocusBridge Project Memory

Last updated: 2026-05-04

## Product

FocusBridge is a local-first attention filter. Android captures phone notifications, filters and prioritizes them, then sends them to a small Tauri desktop app for low-dopamine triage. The Rust relay is optional for later cloud/cross-network sync.

## Current State

- `relay/` is committed and implements the Actix relay, registration, WebSocket routing, queue TTL, health, metrics, and tests.
- `desktop/` is an active untracked scaffold with React, Tauri v2, SQLite schema, QR pairing, priority/study-mode helpers, event-driven UI state, and a local WebSocket receiver in progress.
- `android/` now has its first implementation scaffold: pinned Gradle files, manifest, Hilt app, Room DB, repositories, notification pipeline, priority/study-mode helpers, protocol models, WebSocket client, foreground service, boot receiver, and a basic Compose debug/pairing UI.
- `BUILD_DECISIONS.md` already records intentional Rust/Tauri version deviations from the original playbook.

## Most Recent Work

- Current local slice fixes the first-run Android basics: responsive compact layout, Android 13+ notification runtime prompt, notification-listener status card, CameraX/ZXing QR scanner with camera permission request, manual pairing fallback that starts sync, cleartext local WS permission for MVP LAN pairing, and proper Material icons/buttons across APK navigation and setup actions.
- Latest sync hardening fixes two concrete local-connection blockers: Android foreground sync now reconnects on every service start command after pairing is saved, and Android stores/tries multiple desktop endpoint candidates from the QR instead of failing forever on one stale/wrong Windows adapter IP.
- Desktop QR now advertises multiple local IPv4 candidates, refreshes when the desktop window regains focus or the QR is near expiry, and includes a manual `Refresh QR / network` button for Wi-Fi/hotspot changes.
- The committed `image.png` logo is now used in Android launcher resources, Android app header, desktop sidebar, and desktop PNG bundle/tray assets.
- Both desktop and Android now support clearing old notification history for 1 day, 7 days, 1 month, or custom day counts. Desktop clears SQLite through Tauri; Android clears Room locally.
- Current bug-fix slice adds explicit `Clear all` controls on desktop and Android, and age-based clearing now checks the earlier of phone timestamp and receive timestamp so old phone notifications are not kept just because they arrived recently.
- Desktop Study lane selection now also enables and persists Study Mode instead of only changing the visible filter.
- Desktop icons were regenerated from `image.png` with `pnpm tauri icon ..\image.png`, including the Windows `.ico`; tray template rendering was disabled so the tray icon is not forced into a black mask.
- Windows desktop notification identity now sets an explicit AppUserModelID at startup. Packaged/installed builds should identify as FocusBridge; dev launches can still be affected by Windows notification cache or terminal-launched process identity.
- Android Pair screen is scrollable so CameraX QR scanning no longer squeezes the manual pairing field/save button on short screens, and the mobile header constrains the logo/name row to one line.
- Added `docs/security-and-rules-next-plan.md` for the next production slice: local WSS with certificate pinning, message-level encrypted envelopes, phone app inventory, desktop-managed app/category/rule controls, and relay-safe encrypted sync.
- Current rules slice adds `APP_INVENTORY`, `RULES_UPDATE`, and `RULES_ACK` to the shared protocol model. Android now sends launchable phone app inventory after WebSocket auth succeeds. Desktop stores the app inventory in SQLite, categorizes apps, and exposes Phone app controls in Settings for Mute, Priority, and Study lanes.
- Desktop notification filtering now honors app-level mute, priority, and study-safe controls immediately on the desktop side. This gives the user a functional control surface while the next slice adds true outbound rules sync from desktop to phone.
- Phone app inventory now includes small app icon data URLs. Desktop stores these icons and renders them in the Phone app controls list, falling back to app initials only when Android cannot provide an icon.
- Fixed the desktop dev startup crash `PluginInitialization("sql", "migration 1 was previously applied but has been modified")` by keeping Tauri SQL migrations empty and relying on `db::store::init` for idempotent runtime schema creation/upgrades. Root cause was modifying checksum-tracked migration version `1` after it had already been applied on local machines.
- Removed the unused Tauri SQL plugin entirely after local dev exposed the follow-up `migration 1 was previously applied but is missing` panic. Desktop SQLite is now owned by the Rust `rusqlite` store only.
- Desktop app-rule toggles now send `RULES_UPDATE` to the authenticated phone over the active WebSocket. Android persists app rules in Room, stores synced keyword/contact lists, sends `RULES_ACK`, and enforces muted apps, priority apps, study-safe apps, blocked keywords, priority keywords, and favorite contacts before saving/sending captured notifications.
- Phone app controls now group user-facing phone apps into clearer categories, show priority/blocked word editors in the same section, and avoid listing ordinary built-in system launchers as controllable apps. Selecting the desktop Study lane no longer silently enables Study Mode.
- Desktop and Android notification logs now support deleting individual notifications and deleting age sections such as just now, last hour, today, week, month, and older. Desktop tray config was reduced to the Rust tray only to avoid duplicate tray icons.
- Desktop local pairing now sends explicit `AUTH_OK` / `AUTH_FAILED` WebSocket messages so Android marks the device green only after the desktop accepts the pairing key.
- Current security slice adds local WSS on desktop `:9173` using a persisted self-signed certificate in the desktop app-data directory. Pairing QR payloads now advertise `wss://` LAN candidates and the persisted certificate fingerprint.
- Android WebSocket pairing now pins the desktop certificate fingerprint from the QR before accepting local WSS, and message traffic after auth is wrapped in AES-GCM `ENCRYPTED` envelopes derived from the pairing key.
- Current connection regression fix: the desktop listener now sniffs the first TCP byte and accepts both WSS and legacy plaintext WS on `:9173`. New QR scans still use WSS/pinning, but old Android pairings/manual `ws://` entries no longer fail against a TLS-only listener.
- Desktop tray creation now explicitly assigns the app's bundled default window icon to the tray icon so Windows should show the FocusBridge logo instead of a blank/default tray asset.
- Desktop has a local app-lock gate: first launch creates a PBKDF2-HMAC-SHA256 app password, later launches require unlock before showing notification data/settings.
- Relay now has durable account auth endpoints: `/auth/register`, `/auth/login`, and `/auth/google`. Password users are stored in a local JSON account store, sessions are signed bearer tokens, and relay `/register` now requires a bearer token before creating a device pair.
- Google OAuth client configuration is stored locally in ignored `.env.local`; this file is not committed. The relay now loads `.env.local` / `../.env.local` before config parsing, so the Google endpoint actually sees the local client ID and verifies Google ID tokens against it through Google tokeninfo.
- Desktop now has a user-facing Google OAuth PKCE sign-in flow in the auth gate. It opens Google in the browser, listens on a loopback callback, exchanges the authorization code for a Google ID token, sends that token to relay `/auth/google`, and stores the relay auth session in desktop SQLite settings.
- Added `.env.example` documenting the local Google client ID / relay token-secret configuration split. The real `.env.local` remains ignored and contains the local Google OAuth client ID.
- Current connection regression fix expands Android local pairing candidates from WSS to WSS-first plus legacy WS fallback, so saved/QR pairings can recover if certificate pinning or older plaintext local mode is the immediate blocker.
- Android `SyncEngine.send` now reconnects to the active pairing before sending a captured notification and retries once after a failed send, instead of saving the notification locally and silently dropping desktop delivery when the socket is disconnected.
- Android phone app inventory now uses installed applications, filters out ordinary system internals, keeps launchable/user-facing system apps, and includes app icons. This should fix the desktop showing only one app in Phone app controls on devices where launcher-query visibility was too narrow.
- Desktop Phone app controls were moved out of the right settings column and into the left navigation/sidebar under the filter/security area, with the filter nav no longer taking all sidebar flex space.
- Desktop Google OAuth callback page now says the code was received, not that sign-in is complete. Token exchange failures now include Google's HTTP status and response body so the actual client/redirect/config error is visible instead of the vague `google token exchange failed`.
- Current reliability rollback makes local QR pairing advertise `ws://` endpoints again, matching the last known working local/hotspot transport. Desktop still accepts WSS, but WSS is no longer the default critical path for local mode.
- Desktop QR refresh now reuses the unexpired pairing device ID/key instead of rotating the key on every focus/refresh. This avoids Android scanning a valid-looking QR that becomes invalid before auth completes.
- Android now stores the first effective endpoint candidate as the active endpoint, so stale WSS QR payloads that expand to WS-first migration candidates no longer try the stale WSS endpoint first.
- Android only marks encrypted send mode ready for pinned WSS transports. Plain WS fallback sends notification/app-inventory messages in the known-good plaintext envelope format while still accepting encrypted desktop rule updates.
- Desktop App Control is now a real left-nav destination that opens a full main content page with priority words, blocked words, app icons, categories, and toggles instead of being compacted inside the navigation/sidebar.
- Relay now has `/auth/otp/start` and `/auth/otp/verify` for email/password OTP registration/login. Email delivery uses Resend-compatible HTTP and requires `FOCUSBRIDGE_RESEND_API_KEY` plus `FOCUSBRIDGE_OTP_EMAIL_FROM`.
- Desktop auth UI now exposes the email/password/OTP relay flow as a non-Google sign-in option, while keeping local app-lock password as the local vault gate.
- Current persistence fix addresses the 4-5 minute disconnect: desktop now decrypts later encrypted socket messages with the authenticated socket key, not the mutable QR pairing key that can rotate when the QR refreshes near expiry.
- Android foreground sync now runs a persistent 15-second reconnect supervisor while the foreground service is alive, instead of only connecting once on service start or when a notification is sent.
- Android `SyncEngine` serializes reconnect attempts with a mutex and flushes pending notifications after the connection lock is released, avoiding duplicate reconnect races and self-deadlock during pending flush.
- Desktop only clears the active phone sender when the closing socket is still the current sender, so an older socket closing cannot wipe out a newer reconnect.
- Desktop now has native OS notification popups for phone notifications when the main window is hidden, minimized, or unfocused; masked notifications stay masked in the popup body.
- Desktop close behavior is now guarded: clicking the window X opens an in-app prompt with `Run in tray`, `Quit FocusBridge`, and `Cancel`, so background sync is preserved unless the user intentionally quits.
- Android notification processing now reads user-configurable privacy/filter rules from local config: Masked Peek Mode, blocked keywords, priority keywords, and favorite contacts.
- Desktop notification cards now support Masked Peek Mode: hidden notifications show a masked chip until hover, focus, or click reveals the message.
- Latest local APK output: `android/app/build/outputs/apk/debug/app-debug.apk`.
- Normalized desktop core protocol to the playbook envelope shape: `version`, `type`, `payload`.
- Updated `shared/protocol.json` to use `phone`/`desktop` roles and protocol priority enums.
- Added desktop native app state, SQLite helper functions, and DB-backed notification/settings/pairing commands.
- Replaced the desktop WebSocket stub with a TCP WebSocket listener on `0.0.0.0:9173` that parses FocusBridge envelopes, persists notifications, handles dismissals, and emits Tauri events.
- Updated the React app to listen for native connection, notification, and dismissal events.
- QR pairing now uses a LAN IP discovery attempt instead of always returning `127.0.0.1`.
- Added Android MVP scaffold for notification capture, local persistence, basic pairing input, and WebSocket sync.
- Installed Gradle 8.7 in `C:\tmp\gradle-8.7`, generated `android/gradlew(.bat)`, installed Android SDK command-line tools/platform 34/build-tools 34.0.0/platform-tools in `C:\tmp\android-sdk`, and configured ignored `android/local.properties`.
- Added GitHub Actions for Android, desktop, and relay CI.
- Fixed initial CI failures by committing Rust lockfiles, making `android/gradlew` executable for Linux runners, using portable Rust `stable` toolchains, updating GitHub Actions to current Node-24-compatible major versions, disabling desktop matrix fail-fast, and running `pnpm build` before desktop `cargo check --locked` so Tauri has `frontendDist`.
- Current working tree has an in-progress UI/product polish slice: desktop now has a warm glass “quiet command” layout, animated red/green connection blinker, DB hydration command for persisted notifications, DB-backed triage actions, richer pairing/settings panels, and Android has a tabbed Home/Pair/Rules/Log Compose shell with a live WebSocket connection blinker.
- Added `docs/phase-audit.md` as the current source of truth for which playbook phases are complete, partial, or still blocked before production.

## Known Gaps

Recent connectivity progress:
- The latest pushed UI/product polish commit (`e12f0ad`) is green in GitHub Actions for Android, desktop, and relay.
- Desktop QR payloads now serialize Android-compatible camelCase fields (`relayUrl`, `devicePairId`, `deviceId`, `pairingKey`, `certFingerprint`).
- Android can turn cloud QR payloads into relay WebSocket endpoints with the playbook `phone` role, and the relay accepts both legacy `android` and playbook `phone` roles.
- Android `SyncEngine` now waits for the WebSocket `CONNECTED` state before flushing queued notifications, reducing the risk of losing the first pending notifications after pairing or service startup.

- Local WSS, Android certificate pinning, and pairing-key-derived message encryption are implemented for local mode. This is real encrypted envelope protection, but it is not yet an ECDH/PFS handshake.
- Desktop app controls now sync to Android via `RULES_UPDATE`; Android persists and enforces app, keyword, and contact rules before sending captured notifications.
- Desktop user-facing Google sign-in is implemented with PKCE. Android Google sign-in is not implemented yet because Android needs a Google OAuth Android client/app signing configuration separate from the desktop loopback client.
- Desktop notification list now has code to hydrate existing SQLite notifications on launch, but the latest UI slice still needs verification before it is committed.
- Android QR scanning now exists in the Pair tab, but still needs physical-device verification against the live desktop QR on the same Wi-Fi.
- Cloud relay endpoint construction now exists on Android, but desktop still needs relay registration/client mode and the QR generator still emits local-only payloads until relay settings are wired.
- Android WebSocket local mode now prefers WSS with certificate pinning from the QR. Desktop keeps legacy WS compatibility only to avoid breaking already-saved pairings from older builds.
- Local verification previously hit environment permission blockers: Cargo could not open stale `target/.cargo-lock`, and Vitest/esbuild could not spawn in the sandbox.
- Android `./gradlew.bat testDebugUnitTest lint assembleDebug` passes locally with the installed SDK/cache.
- Android Gradle verification should use the real user Gradle cache on this PC: `GRADLE_USER_HOME=C:\Users\DSU\.gradle`. The previous project-local `android\.gradle` cache was incomplete and missed `org.jetbrains.kotlin:kotlin-serialization-compiler-plugin-embeddable:1.9.24`; the user cache contains it.
- Repo storage cleanup removed ignored generated build outputs from `desktop/target`, `desktop/src-tauri/target`, `relay/target`, and `android/app/build`, reducing the project from about 35.7 GB to about 1.5 GB. These folders are safe to regenerate with normal build commands.
- Android keeps `QUERY_ALL_PACKAGES` intentionally for the full phone app inventory feature. The manifest has a targeted lint suppression for `QueryAllPackagesPermission`; if publishing to Google Play, this permission requires a policy justification or the inventory feature must be redesigned around launcher-visible and notification-discovered apps.
- Production release planning is split into v1.0 LAN/hotspot and v1.1 relay/different-network. The design is saved in `docs/superpowers/specs/2026-05-04-production-release-design.md`; the implementation plan is saved in `docs/superpowers/plans/2026-05-04-production-release.md`.
- v1.0 heartbeat hardening is implemented: Android sends app-level `PING` every 20 seconds after auth, desktop responds with `PONG`, and Android closes stale sockets after 60 seconds without `PONG` so the existing reconnect supervisor can recover the LAN/hotspot session.
- v1.0 notification delivery now uses desktop storage ACKs. Desktop sends `NOTIFICATION_ACK` only after SQLite upsert succeeds; Android marks notifications `SENT` only after that ACK, leaving unacknowledged notifications in the Room pending queue for reconnect retry.
- Desktop v1.0 diagnostics are implemented in Settings. The panel shows connected/disconnected state, transport (`wss` or legacy `ws_legacy`), LAN port 9173, endpoint candidates, certificate fingerprint, last heartbeat, auth failures, and disconnect reason.
- Desktop Settings now includes a Windows setup action that launches a UAC-protected firewall rule helper for FocusBridge local sync on TCP 9173. Packaged builds use the `com.focusbridge.desktop` AppUserModelID and FocusBridge bundle metadata for native toast identity; dev runs launched from PowerShell can still show shell identity due to Windows shortcut/toast caching.
- Android v1.0 onboarding now shows a Production setup checklist for notification access, Android notification permission, battery optimization, desktop pairing, and live connection. The battery action opens Android battery optimization settings so users can prevent OEM idle disconnects.
- FocusBridge v1.0.0 artifacts were built and copied to `C:\Users\DSU\Desktop\Foucs\FocusBridge-v1.0.0-release`: Windows MSI, Android release APK, Android debug APK, and a short install README. The Android release APK is locally test-signed with the debug signing config; replace with a private release keystore before public app-store distribution.
- Latest v1.0.0 hotfix addresses production lag/terminal flashing and duplicate delivery: Windows `ipconfig` diagnostics now run with `CREATE_NO_WINDOW`, desktop diagnostics polling is reduced to 30 seconds, and Android reconnects during a direct notification send without flushing the same pending notification first. Rebuilt artifacts were copied to `C:\Users\DSU\Desktop\Foucs\FocusBridge-v1.0.0-release`.
- Latest duplicate-notification fix also removes Android's random UUID suffix from notification IDs and uses Android's stable notification key instead. Desktop now only shows native Windows toasts for new notification IDs, while updates to the same notification upsert the existing row. Local app-lock accepts 8+ character passwords or 4+ digit PINs, and Settings now supports an idle auto-lock timeout with default unlimited.
- Firebase Auth setup started. Android `google-services.json` is now placed at `android/app/google-services.json`, Android uses Google Services plugin `4.4.4` plus Firebase Auth through BoM `33.7.0`, and desktop has Firebase Web email/password signup/login in `desktop/src/lib/firebaseAuth.ts`. BoM `34.12.0` was intentionally not used because it pulls Kotlin 2.x metadata incompatible with the current Kotlin `1.9.24` toolchain.
- Desktop auth now uses an account-first flow: Firebase login/signup or guest mode, a 90-day app-enforced account session, then local PIN/password unlock. New local lock setup requires a security question/answer, and local forgotten PIN/password reset verifies the hashed security answer before allowing a new PIN/password.
- Android account UI is not yet gated by Firebase/local PIN because the verified Android notification sync path should not be destabilized in the desktop auth patch. Treat Android auth as a separate production hardening task.
- Latest connection-state fix: desktop no longer trusts only the last socket event. React polls diagnostics every 3 seconds and downgrades stale heartbeats; the desktop websocket server emits disconnected after 45 seconds without phone traffic; Android heartbeats every 10 seconds, times out missing PONGs after 25 seconds, and marks itself disconnected if a send is refused.
- Local "Forgot PIN/password" is clickable even for older installs without a recovery question. If recovery was never configured, the recovery panel now exposes that state instead of hiding the path behind a disabled button.
- Desktop `pnpm tsc --noEmit`, `pnpm vitest run`, `pnpm build`, and Rust `cargo check` pass locally. Vite/Vitest need elevated execution in this environment because esbuild spawn is blocked by the sandbox.
-  Error: I see the same message twice. Fix this. Sam emessage is being pushed twice both in mobile and desktop.
- The Tauri GUI crate disables Cargo's native lib test harness because it has no unit tests and the Windows harness can crash while loading native WebView/Tauri symbols; desktop Rust behavior tests live in `desktop/core`, and `cargo check --locked` still compiles the Tauri crate.
- Relay `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass locally.
- After switching Rust toolchain files to portable `stable`, this Windows shell may select `stable-x86_64-pc-windows-msvc`; relay compile gates fail locally if Git's Unix `link.exe` shadows the Visual Studio linker. Use `rustup run stable-x86_64-pc-windows-gnu cargo ...` for relay checks on this PC, or repair the MSVC build tools/PATH later.
## Next Best Tasks

1. Install the debug APK on a physical Android phone, launch desktop Tauri, scan the QR, and record real pairing/notification delivery results in `docs/integration-log.md`.
2. Verify native Windows notification toast behavior and close-to-tray behavior manually in a running Tauri desktop session.
3. Add Android Google sign-in only after creating the Android OAuth client in Google Cloud with the app package name and signing certificate SHA-1/SHA-256.
4. Implement true different-network sync by wiring desktop into the existing relay registration/client path; direct LAN QR cannot cross NAT or isolated guest/hotspot networks by itself.
5. Upgrade message encryption to an authenticated ECDH handshake with forward secrecy if the product needs stronger E2E guarantees than pairing-key-derived AES-GCM.
