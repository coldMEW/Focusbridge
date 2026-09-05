# Network and security audit checkpoint

This is an initial source audit, not certification of the whole application.
Baseline: 9869810. Reviewed recent connection commits, local WebSocket server,
Android pairing/client/supervisor, notification queries, crypto envelopes, and
relay configuration/routing entry points.

## Changes in this checkpoint

- Android Captured/Priority counters now query the full stored history; the
  recent-list limit remains 100 to avoid loading all messages into the UI.
- Reject application messages before socket authentication and empty auth keys.
- Reject unsupported protocol versions and expired new-pairing sessions.
- Validate AES-GCM nonce length before constructing the Rust nonce value.
- Run desktop socket cleanup after read, parse, decrypt, and send errors. Only
  the owning socket may clear the current connection and emit disconnected.
- Retry unacknowledged Android records during connected supervisor cycles.
- Preserve coroutine cancellation and prevent sends during persisted manual pause.
- Ignore callbacks from obsolete Android sockets.
- Local pairing uses pinned WSS with no plaintext downgrade. Existing saved
  ws endpoints are upgraded using the stored certificate fingerprint. Both
  clients must be updated; missing/invalid fingerprints require re-pairing.
- Legacy cloud mode is blocked on Android pending a secure relay protocol.

## Open release blockers

1. The relay receives pairing_key, which is also the message-encryption secret.
   Therefore the current protocol does not protect content from the relay.
   Separate per-role relay capabilities from the end-to-end secret; never put
   the latter in registration, URL query strings, logs, or relay storage.
2. Desktop relay client and cross-network acceptance tests remain unfinished.
3. Message encryption needs replay protection and a versioned session handshake.
4. Mobile app lock needs atomic credential loading, background relock behavior,
   attempt throttling, masked inputs, and stronger recovery/storage review.
5. Background service holds unlimited locks; dataSync lifecycle, OEM behavior,
   Doze, foreground startup restrictions, and manual-pause races need tests.
6. Relay account/OTP endpoints need abuse limits, durable pairing ownership,
   revocation, bounded queues, and server-restart tests before exposure.
7. Android release still uses debug signing; Docker uses an old Rust image.
8. Real-device overnight, network-switch, packet-loss, and duplicate-update tests
   are outstanding. Compilation does not establish connection reliability.

## Cross-network design

Use direct pinned WSS when reachable and a public WSS relay on TCP 443 otherwise.
Both devices initiate outbound connections. Use opaque pair IDs and separate
phone/desktop authentication capabilities. Encrypt on the originating device,
decrypt only on the destination. Add bounded durable ACK/retry and deduplication.
Consider HTTPS polling only if WebSocket restrictions are demonstrated.
DNS resolves the relay address; it cannot override client isolation or firewalls.
No design guarantees access on networks that block the relay or all Internet.

## Hosting from scratch

See `free-relay-options.md` for free-tier comparisons and account setup from
scratch. The user requires free-first infrastructure. The initial paid Render
suggestion is superseded; no service has been deployed or purchased.

Deployment files and a live deployment are not provided by this checkpoint.
Do not expose the existing relay as a production service yet.

References:
- https://render.com/docs/web-services
- https://render.com/docs/websocket
- https://render.com/docs/disks
- https://developer.android.com/training/monitoring-device-state/doze-standby
- https://developer.android.com/develop/background-work/services/fgs/service-types
