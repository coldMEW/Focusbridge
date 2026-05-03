# FocusBridge Phase Audit

Last updated: 2026-05-03

This audit compares the current repository against the playbook phases. It is intentionally direct so the next build step is obvious.

## Phase 1: Android Notification Capture

Status: partial MVP implemented.

Implemented:
- Kotlin, Compose, Hilt, Room scaffold.
- `NotificationService` using `NotificationListenerService`.
- `NotificationFilter`, `NotificationProcessor`, `DefaultParser`.
- Room entities, DAOs, repositories, and recent notification UI log.
- Foreground service and boot receiver scaffolding.

Missing before production:
- Physical-device notification access acceptance test.
- Rich onboarding checklist for notification access and OEM battery guidance.
- Per-app parser set beyond default parser.

## Phase 2: Local Sync

Status: partial MVP implemented.

Implemented:
- Android OkHttp WebSocket client and sync engine.
- Desktop Tauri local WebSocket listener on port `9173`.
- Protocol envelope shape across shared, desktop, and Android.
- Desktop SQLite persistence and live Tauri events.
- Manual QR payload pairing and local endpoint storage.

Missing before production:
- Local transport is still plain `ws://`, not pinned `wss://`.
- Android QR camera scanning is not implemented.
- Reconnection needs stronger condition-based flushing after socket open.
- End-to-end physical-device latency test has not been recorded.

## Phase 3: Desktop Triage

Status: mostly implemented for MVP.

Implemented:
- Modern desktop notification cards with Ignore and Pin actions.
- DB-backed `mark_ignored`, `mark_important`, and notification hydration.
- Filters for inbox, priority, study lane, and security/2FA.
- Phone dismissal messages remove notifications from desktop.
- Red, green, amber, and grey connection indicator in desktop UI.

Missing before production:
- Keyboard shortcuts.
- Notification cleanup and retention policy.
- Desktop-to-phone action sync for triage state.

## Phase 4: Study Mode

Status: partial foundation implemented.

Implemented:
- Android priority and urgency rule classes.
- Desktop Study Mode toggle and Study/2FA filters.
- Android Rules tab and Study Mode config persistence.

Missing before production:
- Batch manager and timed batch delivery.
- Favorite contacts and priority keyword editing UI.
- End-to-end Study Mode filtering acceptance test.
- Batch expand/collapse UI.
- Display delay feature.

## Phase 5: Polish and Parsers

Status: active UI polish started.

Implemented:
- Desktop warm glass "quiet command" cockpit with animated cards and status panels.
- Android tabbed Home/Pair/Rules/Log Compose shell with live connection indicator.
- System tray scaffold exists on desktop.

Missing before production:
- Major app parsers: WhatsApp, Telegram, Instagram, Signal, Gmail, Slack, Discord.
- Parser unit tests.
- Full OEM optimization guide inside Android app.
- 48-hour stability test.

## Phase 6: Cloud Relay and Cross-Network Sync

Status: relay server implemented, clients not wired.

Implemented:
- Rust relay with auth, WebSocket routing, queue TTL, metrics, Docker build, and CI.

Missing before production:
- Desktop relay client mode is not connected to UI/settings.
- Android cloud relay mode is not connected to pairing/settings.
- QR pairing does not yet advertise both local and relay endpoints.
- Hosted relay deployment and real cross-network acceptance test.

## Phase 7: Deep Focus / AI / Paid Features

Status: intentionally deferred.

Missing:
- AI urgency detection.
- Subscription/license system.
- Notification analytics and focus sessions.

## Current Production Blockers

1. Replace local `ws://` with pinned local `wss://`.
2. Add Android QR camera scanning.
3. Wire cloud relay client mode on Android and desktop.
4. Add device acceptance log with real APK on a physical Android phone and desktop GUI on Windows.
5. Implement Study Mode batching and desktop-to-phone action sync.
6. Add per-app parsers and parser tests.
