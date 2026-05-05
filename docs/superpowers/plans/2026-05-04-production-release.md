# Production Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make FocusBridge v1.0 production-ready for Windows + Android LAN/hotspot usage, then prepare v1.1 relay work without breaking local mode.

**Architecture:** v1.0 hardens the existing desktop-hosted WebSocket path with heartbeat, retry, diagnostics, packaging, and onboarding. v1.1 adds relay as a second transport after local mode is stable.

**Tech Stack:** Tauri v2, React/Vite, Rust, SQLite/rusqlite, Kotlin, Compose, Hilt, Room, OkHttp WebSocket, Android Gradle Plugin 8.4.

---

### Task 1: Local Connection Heartbeat

**Files:**
- Modify: `shared/protocol.json`
- Modify: `desktop/core/src/handler.rs`
- Modify: `desktop/src-tauri/src/server/ws_server.rs`
- Modify: `android/app/src/main/java/com/focusbridge/android/sync/Protocol.kt`
- Modify: `android/app/src/main/java/com/focusbridge/android/sync/WebSocketClient.kt`

- [x] **Step 1: Add protocol helpers**

Add Android `Protocol.ping()`, `Protocol.pong()`, and `Protocol.status(...)` helpers. Add desktop `PONG` response when receiving `PING`.

- [x] **Step 2: Add Android heartbeat loop**

Start a coroutine after `AUTH_OK` that sends `PING` every 20 seconds. Track the last `PONG`; if no `PONG` arrives within 60 seconds, close the socket so `SyncEngine` reconnects.

- [x] **Step 3: Verify**

Run:

```powershell
cmd /c "set GRADLE_USER_HOME=C:\Users\DSU\.gradle&& gradlew.bat testDebugUnitTest lint assembleDebug"
```

Expected: `BUILD SUCCESSFUL`.

Run desktop core tests:

```powershell
cargo test
```

Expected: handler/protocol tests pass.

### Task 2: Notification ACK And Retry Queue

**Files:**
- Modify: `shared/protocol.json`
- Modify: `desktop/core/src/protocol.rs`
- Modify: `desktop/src-tauri/src/server/ws_server.rs`
- Modify: `android/app/src/main/java/com/focusbridge/android/sync/Protocol.kt`
- Modify: `android/app/src/main/java/com/focusbridge/android/sync/WebSocketClient.kt`
- Modify: `android/app/src/main/java/com/focusbridge/android/sync/SyncEngine.kt`

- [x] **Step 1: Add `NOTIFICATION_ACK`**

Define payload `{ "id": string, "accepted": boolean, "serverTime": integer }`.

- [x] **Step 2: Desktop sends ACK after store**

After `store::upsert_notification` succeeds, desktop sends `NOTIFICATION_ACK` over the same authenticated socket.

- [x] **Step 3: Android marks sent only on ACK**

Change Android so `send()` does not call `markSent` immediately after socket send. It marks sent when ACK arrives. Pending notifications remain in Room until ACK.

- [ ] **Step 4: Verify retry**

Manual test: pair phone, kill desktop, generate notification, restart desktop, verify pending notification arrives once and is marked sent.

### Task 3: Desktop Diagnostics

**Files:**
- Modify: `desktop/src-tauri/src/server/ws_server.rs`
- Modify: `desktop/src-tauri/src/commands.rs`
- Modify: `desktop/src/App.tsx`
- Modify: `desktop/src/styles.css`

- [x] **Step 1: Add diagnostics command**

Expose LAN IP candidates, port `9173`, connection state, last heartbeat, last auth failure, and active transport.

- [x] **Step 2: Add UI panel**

Add a Diagnostics section in desktop settings/app control area with green/yellow/red states and actionable messages.

- [x] **Step 3: Verify**

Run `pnpm tsc --noEmit`, `pnpm vitest run`, and `pnpm build`.

### Task 4: Android Reliability Onboarding

**Files:**
- Modify: `android/app/src/main/java/com/focusbridge/android/MainActivity.kt`
- Modify: Android Compose screen files under `android/app/src/main/java/com/focusbridge/android/ui`

- [x] **Step 1: Show required setup checklist**

Checklist items: notification access, camera permission, foreground sync running, battery optimization warning, active pairing.

- [x] **Step 2: Add battery optimization action**

Open Android battery optimization settings for FocusBridge.

- [x] **Step 3: Verify**

Run Android tests/lint/build and install APK on phone.

### Task 5: Windows Release Packaging

**Files:**
- Modify: `desktop/src-tauri/tauri.conf.json`
- Modify: `desktop/package.json`
- Modify: `.github/workflows/desktop-ci.yml`
- Modify: `docs/release-checklist.md`

- [x] **Step 1: Ensure icons and bundle metadata**

Verify product name, executable name, app identifier, Windows icon, tray icon, and notification identity.

- [x] **Step 1a: Add first-run Windows firewall helper**

Settings includes a Windows setup action that requests elevation and adds an inbound TCP 9173 allow rule for the current FocusBridge executable.

- [x] **Step 2: Add release build checklist**

Document `pnpm tauri build`, installer output path, smoke test steps, and known Windows notification cache behavior.

- [x] **Step 3: Verify**

Run `pnpm build` and `pnpm tauri build --ci` where local tooling permits.

### Task 6: v1.1 Relay Preparation

**Files:**
- Modify: `docs/superpowers/specs/2026-05-04-production-release-design.md`
- Create: `docs/relay-v1.1-plan.md`

- [x] **Step 1: Document relay transport boundary**

Write the exact desktop/Android/relay responsibilities without changing local v1.0 behavior.

- [x] **Step 2: Define relay acceptance tests**

List same-network, different-network, and relay-down expected behavior.

---

## Verification Gates

- Android: `cmd /c "set GRADLE_USER_HOME=C:\Users\DSU\.gradle&& gradlew.bat testDebugUnitTest lint assembleDebug"`
- Desktop frontend: `pnpm tsc --noEmit`, `pnpm vitest run`, `pnpm build`
- Desktop Rust/core: `cargo test` in `desktop/core`
- Relay: `cargo test` in `relay`
- Manual: physical Android notification delivery to Windows desktop over hotspot for at least 15 idle minutes.
