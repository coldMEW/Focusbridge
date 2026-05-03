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

- Normalized desktop core protocol to the playbook envelope shape: `version`, `type`, `payload`.
- Updated `shared/protocol.json` to use `phone`/`desktop` roles and protocol priority enums.
- Added desktop native app state, SQLite helper functions, and DB-backed notification/settings/pairing commands.
- Replaced the desktop WebSocket stub with a TCP WebSocket listener on `0.0.0.0:9173` that parses FocusBridge envelopes, persists notifications, handles dismissals, and emits Tauri events.
- Updated the React app to listen for native connection, notification, and dismissal events.
- QR pairing now uses a LAN IP discovery attempt instead of always returning `127.0.0.1`.
- Added Android MVP scaffold for notification capture, local persistence, basic pairing input, and WebSocket sync.
- Installed Gradle 8.7 in `C:\tmp\gradle-8.7`, generated `android/gradlew(.bat)`, installed Android SDK command-line tools/platform 34/build-tools 34.0.0/platform-tools in `C:\tmp\android-sdk`, and configured ignored `android/local.properties`.
- Added GitHub Actions for Android, desktop, and relay CI.
- Fixed initial CI failures by committing Rust lockfiles, making `android/gradlew` executable for Linux runners, using portable Rust `stable` toolchains, updating GitHub Actions to current Node-24-compatible major versions, and disabling desktop matrix fail-fast.

## Known Gaps

- Desktop local WebSocket is currently plain WS for MVP wiring. WSS with persisted self-signed certs and Android certificate pinning still needs hardening.
- Desktop notification list loads live events, but it does not yet hydrate existing SQLite notifications on launch.
- Android QR scanning is currently manual payload paste; CameraX/ZXing camera scanning still needs UI integration.
- Android certificate pinning is represented by `CertificateManager`, but the WebSocket client is plain WS until desktop WSS hardening is done.
- Local verification previously hit environment permission blockers: Cargo could not open stale `target/.cargo-lock`, and Vitest/esbuild could not spawn in the sandbox.
- Android `./gradlew.bat test lint assembleDebug` passes locally with the installed SDK.
- Desktop `pnpm tsc --noEmit`, `pnpm vitest run`, `pnpm build`, and Rust `cargo check` pass locally. Vite/Vitest need elevated execution in this environment because esbuild spawn is blocked by the sandbox.
- Relay `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass locally.
- After switching Rust toolchain files to portable `stable`, this Windows shell may select `stable-x86_64-pc-windows-msvc`; relay compile gates fail locally if Git's Unix `link.exe` shadows the Visual Studio linker. Use `rustup run stable-x86_64-pc-windows-gnu cargo ...` for relay checks on this PC, or repair the MSVC build tools/PATH later.

## Next Best Tasks

1. Add QR camera scanning with ZXing/CameraX or another scanner integration.
2. Hydrate desktop notifications from SQLite on launch.
3. Harden local desktop transport from WS to WSS with persisted certs and Android pinning.
4. Add desktop-to-phone action handling for important/ignored/study-mode toggles.
5. Test real phone-to-desktop pairing on the same Wi-Fi and record results in `docs/integration-log.md`.
