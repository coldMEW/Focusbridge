# FocusBridge Production Release Design

## Release Split

FocusBridge v1.0 is the local production release. It must work reliably for Windows desktop plus Android phone on the same LAN or hotspot. It does not promise different-network connectivity, because campus Wi-Fi isolation, NAT, guest networks, and firewalls can block direct peer-to-peer traffic. When direct connection is blocked, the app must show a useful diagnostic instead of silently failing.

FocusBridge v1.1 is the relay production release. It adds hosted relay connectivity for different networks and blocked LANs. Relay mode must carry only authenticated and encrypted device messages.

## v1.0 Acceptance

- Android pairs with desktop by QR or manual endpoint on same Wi-Fi/hotspot.
- Desktop and Android show the same connection state within a few seconds.
- Connection survives idle periods through heartbeat and reconnect.
- Phone notifications appear on desktop, including when desktop is minimized or in tray.
- Android queues notifications while disconnected and retries them after reconnect.
- Desktop receives app inventory and syncs app/keyword/contact rules back to Android.
- Android enforces muted apps, blocked keywords, priority rules, and Study Mode rules before sending.
- Desktop diagnostics explain LAN endpoint, active IP candidates, server port, last heartbeat, and recent failure reason.
- Windows builds use FocusBridge icon/name for the executable, tray, and native notifications.
- Android release APK builds from a clean checkout using documented local environment values.

## v1.1 Acceptance

- Relay has a deployable production configuration.
- Desktop can register/connect to relay as a desktop device.
- Android can connect to relay as a phone device.
- Pairing QR includes relay pairing metadata when relay is enabled.
- Different-network sync works through relay with message-level encryption.
- Relay cannot read notification bodies, rules, app inventory labels, or message content.

## Architecture

The local path stays simple: desktop owns the listening WebSocket server on port `9173`; Android owns notification capture and retry. The shared protocol remains the source of truth for message types. Reliability is added with app-level `PING`/`PONG`, `STATUS`, notification ACKs, local Android queue retention, and desktop diagnostics events.

The relay path is isolated from the v1.0 local path. Relay work must not break LAN/hotspot pairing. Relay mode will be added as a second transport selected by pairing payload and user settings.

## Risks

- `QUERY_ALL_PACKAGES` supports full app controls but requires Google Play policy justification.
- Windows native notifications may cache old AppUserModelID/icon values in dev runs; installed builds are the release acceptance target.
- University Wi-Fi can block LAN device discovery or peer connections; v1.0 must detect this and recommend hotspot or v1.1 relay.
- Android OEM battery policies can stop background work; onboarding must guide users to disable battery optimization for reliable sync.
