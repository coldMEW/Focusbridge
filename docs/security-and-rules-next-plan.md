# FocusBridge Secure Sync and Rules Plan

Last updated: 2026-05-03

## Goal

Make FocusBridge production-safe for local and cross-network sync while giving the user desktop-controlled notification rules: app allow/block, app priority, Study Mode inclusion, priority keywords, blocked keywords, and favorite contacts.

## Current Findings

- Hotspot/data success proves the phone-to-desktop sync path works.
- University Wi-Fi failure is likely network client isolation or inbound port filtering. Direct LAN WebSocket cannot guarantee connectivity on protected campus or guest networks.
- Different-network support should use relay mode. Direct QR LAN mode remains best for same private Wi-Fi/hotspot.
- Current local transport is authenticated plain WebSocket. This is acceptable only for MVP debugging, not production.

## Recommended Security Architecture

1. Local mode should move from `ws://` to `wss://`.
2. Desktop should generate and persist a self-signed certificate on first run.
3. Pairing QR should include the certificate SHA-256 fingerprint.
4. Android should reject local WSS unless the server certificate fingerprint matches the QR.
5. Message payloads should also be encrypted with a per-pairing session key derived from the QR pairing secret plus an ephemeral handshake.
6. Relay mode should forward only encrypted envelopes. The relay must not be able to read notification title, sender, body, rules, or app inventory.

## Recommended Rules Architecture

Android remains the enforcement point because it sees notifications first.

Desktop should become the comfortable control surface:
- Show installed/captured app inventory from phone.
- Categorize apps locally by package/name heuristics: messaging, email, calendar, finance, school/work, social, shopping, media, system, other.
- Let user choose per app: allow, mute, priority, study-safe, 2FA/security lane.
- Let user define priority keywords, muted keywords, favorite contacts, and privacy masking defaults.
- Sync rule updates to Android over the paired encrypted channel.

Android should:
- Send `APP_INVENTORY` after pairing and when new apps appear.
- Store `NotificationRuleEntity` and apply rules before sending notifications.
- Reclassify priority using explicit user rules first, then heuristics.
- Keep blocked notifications local or discard them according to user setting.

## Protocol Additions

- `APP_INVENTORY`: phone to desktop, includes package name, label, category guess, last seen timestamp, notification count.
- `RULES_UPDATE`: desktop to phone, includes app rules, keywords, contacts, masking preferences, Study Mode policy.
- `RULES_ACK`: phone to desktop, confirms applied version.
- `SECURE_HELLO`: both directions, exchanges ephemeral public keys and proves possession of pairing secret.
- `ENCRYPTED`: outer envelope for encrypted payloads after secure session starts.

## Implementation Order

1. Persist desktop certificate and switch local server to WSS.
2. Add Android certificate pinning against QR fingerprint.
3. Add secure session handshake and encrypted message wrapper.
4. Add Android app inventory collection and `APP_INVENTORY`.
5. Add desktop app/rule DB tables and UI.
6. Add `RULES_UPDATE` and Android rule enforcement.
7. Wire relay mode only after encrypted envelopes are working locally.

## Non-Goals For This Slice

- No AI classification.
- No cloud account system.
- No payment tier.
- No store release packaging until WSS, pinning, and encrypted relay envelopes pass.
