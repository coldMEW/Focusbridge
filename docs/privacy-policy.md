# Privacy Policy (MVP draft)

FocusBridge is local-first. Defaults:

- No account, no sign-up.
- No telemetry, no analytics.
- No crash reporting without explicit opt-in (not yet implemented).

## Data stored

- **Android Room DB**: notifications + pairing metadata + config. Local to the device.
- **Desktop SQLite**: notifications + pairing metadata + settings. Local to the desktop.
- **Relay server**: nothing. Messages live in-memory for max 5 min / 100 per pair, then dropped.

## Data in transit

- Local mode: WSS with self-signed cert pinned by QR fingerprint.
- Cloud mode: WSS with CA-verified TLS to relay.

## Third parties

- None in MVP. Phase 7 (out of scope in this build) would add optional AI provider with explicit opt-in.
