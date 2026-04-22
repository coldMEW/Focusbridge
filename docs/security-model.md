# Security Model

Authoritative ref: `FocusBridge_Development_Plan.md` §20.

## Summary

| Threat | Mitigation |
|---|---|
| WiFi sniffing | WSS (TLS) even on LAN |
| Local MITM | Cert fingerprint pinned via QR payload |
| Relay snooping | Zero-storage in-memory router; open-source |
| Pairing-key theft | 256-bit random, kept in Android Keystore / OS keychain |
| QR replay | 5 min expiry; single-use |

## Non-goals (MVP)

- E2E encryption above TLS (Phase 3 future).
- SQLCipher at-rest encryption (Phase 2 future).
- Code-signing notarization (release phase).
