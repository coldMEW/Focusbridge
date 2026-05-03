# FocusBridge Project Memory

Last updated: 2026-05-03

## Product

FocusBridge is a local-first attention filter. Android captures phone notifications, filters and prioritizes them, then sends them to a small Tauri desktop app for low-dopamine triage. The Rust relay is optional for later cloud/cross-network sync.

## Current State

- `relay/` is committed and implements the Actix relay, registration, WebSocket routing, queue TTL, health, metrics, and tests.
- `desktop/` is an active untracked scaffold with React, Tauri v2, SQLite schema, QR pairing, priority/study-mode helpers, event-driven UI state, and a local WebSocket receiver in progress.
- `android/` now has its first implementation scaffold: pinned Gradle files, manifest, Hilt app, Room DB, repositories, notification pipeline, priority/study-mode helpers, protocol models, WebSocket client, foreground service, boot receiver, and a basic Compose debug/pairing UI.
- `BUILD_DECISIONS.md` already records intentional Rust/Tauri version deviations from the original playbook.

## Most Recent Work

- Current local slice fixes the first-run Android basics: responsive compact layout, Android 13+ notification runtime prompt, notification-listener status card, CameraX/ZXing QR scanner with camera permission request, manual pairing fallback that starts sync, cleartext local WS permission for MVP LAN pairing, and proper Material icons/buttons across APK navigation and setup actions.
- Desktop local pairing now sends explicit `AUTH_OK` / `AUTH_FAILED` WebSocket messages so Android marks the device green only after the desktop accepts the pairing key.
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

- Desktop local WebSocket is currently plain WS for MVP wiring. WSS with persisted self-signed certs and Android certificate pinning still needs hardening.
- Desktop notification list now has code to hydrate existing SQLite notifications on launch, but the latest UI slice still needs verification before it is committed.
- Android QR scanning now exists in the Pair tab, but still needs physical-device verification against the live desktop QR on the same Wi-Fi.
- Cloud relay endpoint construction now exists on Android, but desktop still needs relay registration/client mode and the QR generator still emits local-only payloads until relay settings are wired.
- Android certificate pinning is represented by `CertificateManager`, but the WebSocket client is plain WS until desktop WSS hardening is done.
- Local verification previously hit environment permission blockers: Cargo could not open stale `target/.cargo-lock`, and Vitest/esbuild could not spawn in the sandbox.
- Android `./gradlew.bat test lint assembleDebug` passes locally with the installed SDK.
- Desktop `pnpm tsc --noEmit`, `pnpm vitest run`, `pnpm build`, and Rust `cargo check` pass locally. Vite/Vitest need elevated execution in this environment because esbuild spawn is blocked by the sandbox.
- The Tauri GUI crate disables Cargo's native lib test harness because it has no unit tests and the Windows harness can crash while loading native WebView/Tauri symbols; desktop Rust behavior tests live in `desktop/core`, and `cargo check --locked` still compiles the Tauri crate.
- Relay `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass locally.
- After switching Rust toolchain files to portable `stable`, this Windows shell may select `stable-x86_64-pc-windows-msvc`; relay compile gates fail locally if Git's Unix `link.exe` shadows the Visual Studio linker. Use `rustup run stable-x86_64-pc-windows-gnu cargo ...` for relay checks on this PC, or repair the MSVC build tools/PATH later.

## Next Best Tasks

1. Install the debug APK on a physical Android phone, launch desktop Tauri, scan the QR, and record real pairing/notification delivery results in `docs/integration-log.md`.
2. Verify native Windows notification toast behavior and close-to-tray behavior manually in a running Tauri desktop session.
3. Harden local desktop transport from WS to WSS with persisted certs and Android pinning.
4. Add desktop-to-phone action handling for important/ignored/study-mode toggles.
5. Add relay registration/client mode in desktop so different-Wi-Fi pairing becomes product-grade rather than local-only plus relay scaffold.
