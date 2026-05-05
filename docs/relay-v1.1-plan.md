# FocusBridge v1.1 Relay Plan

v1.1 adds different-network sync without weakening v1.0 LAN/hotspot mode.

## Transport Boundary

- Android remains the notification capture owner.
- Desktop remains the triage UI and rules control surface.
- Relay routes authenticated encrypted envelopes only.
- Relay must not read notification bodies, rule text, contact names, app labels, or app icons.
- Local LAN/hotspot QR pairing remains available even when relay is configured.

## Required Work

1. Add desktop relay client mode that authenticates with the relay and attaches as role `desktop`.
2. Add Android relay client mode that attaches as role `phone`.
3. Extend QR payloads to include relay URL and device-pair ID when cloud mode is enabled.
4. Keep message-level encryption active before relay forwarding.
5. Add desktop diagnostics for relay connected, relay auth failed, relay unavailable, and LAN fallback active.
6. Add Android diagnostics for relay connected, relay auth failed, relay unavailable, and local fallback active.
7. Evaluate optional phone-call integration for cases where Android does not expose calls as notification-listener events. This must be explicit opt-in because phone/call permissions are sensitive.

## Acceptance Tests

- Same Wi-Fi: local mode connects directly without relay.
- Phone hotspot: local mode connects directly without relay.
- University/guest Wi-Fi with LAN isolation: local mode explains failure and relay mode connects.
- Different networks: relay mode connects and delivers notifications.
- Relay down: both apps show relay unavailable and keep local mode usable.
- Relay privacy: relay logs never contain notification title/body, contact names, rule text, app labels, or app icons.
