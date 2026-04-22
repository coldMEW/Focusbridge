# Architecture

See `FocusBridge_Development_Plan.md` §2 for the authoritative diagram.

## Components

- `android/` — Kotlin + Compose + Hilt + Room. Notification capture, priority engine, sync client.
- `desktop/` — Tauri v2 (Rust + React/TS). WebSocket server (local) + relay client (cloud), SQLite, grayscale UI.
- `relay/` — Rust Actix-web zero-storage WebSocket router.
- `shared/protocol.json` — Wire protocol JSON Schema (single source of truth).

## Modes

1. **Local**: Desktop hosts WSS on `:9173`. Android connects direct. Self-signed cert pinned via QR.
2. **Cloud**: Both clients connect WSS to `relay/`. Relay routes in-memory, no disk.

## Principles

- Local-first default.
- Dopamine-hostile UI: grayscale, no avatars/colors/badges.
- Phone is source of truth. Desktop is a triage view.
- Free tier = rule engine only. AI + payments out of scope (Phase 7 skipped).
