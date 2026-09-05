# Network review follow-up

## Scope and evidence

Reviewed the supplied conversation, recent networking commit history, project
memory, Android inventory/client/supervisor, desktop socket ownership and app
inventory storage, and relay routing entry points. This is not a claim that every
line of every historical version has been audited. Unreviewed areas remain.

## Findings

| Problem | Evidence / action |
| --- | --- |
| Uninstalled apps remain visible | Desktop inventory was UPSERT-only. Reconcile complete snapshots transactionally; preserve rule preferences separately from visibility. |
| Manual pause races with reconnect | Android supervisor and send path can overlap a disconnect. Check session cancellation and persisted pause together, not only UI state. |
| Old desktop sockets still process messages | Cleanup already checks ownership, but inbound processing did not. Reject processing on replaced or manually disconnected sessions. |
| New session inherits old diagnostics | Reset prior heartbeat and auth error on new authenticated sender. |
| Stalled writes prevent disconnect cleanup | Socket writes now time out after 10 seconds; close attempts after 2 seconds. Duplex-socket tests cover delivery and backpressure. |
| Rules can arrive out of order in Android storage | Serial application of received updates; tests delay the first write and cancel old-session queued work. |
| Android total stops at 100 | Prior checkpoint ae95eba uses SQL COUNT aggregates, not the recent-list size. The recent list itself is intentionally still bounded. |
| Different networks cannot connect | Public relay and desktop relay client are not deployed/completed. DNS is not a substitute. Cloud mode remains blocked for safety. |
| Relay falsely reports delivery | relay/src/relay.rs ignores channel send errors and pops pending messages before successful delivery. Needs failure/reattach regression tests before deployment. |
| Relay stale detach can remove newer sender | detach identifies only pair and role, not socket generation. Needs connection ownership before deployment. |
| Relay abuse controls are placeholders | ping_interval and rate_limit_per_min in RelayState are reserved, not enforced there. Do not claim these protections are complete. |
| Relay lacks end-to-end privacy | Pairing secret is supplied to server; separate relay capabilities from device-only encryption secrets. |

## Verification

- Android: installed Gradle 8.7, `testDebugUnitTest assembleDebug --offline`,
  using the existing user Gradle cache: 34 tests passed and debug APK built.
- Desktop frontend: `pnpm vitest run`: 26 tests; `pnpm tsc --noEmit`: passed.
  Vitest required elevated filesystem access for esbuild, not a code workaround.
- Rust MSVC: core tests 24 passed; SQLite inventory tests 7 passed; connection
  state/socket I/O tests 6 passed. The backpressure regression failed before the
  deadline was added and passed afterwards.
- `cargo clippy --offline --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  and `git diff --check`: passed on the final Rust changes.
- Android/Windows hardware tests, overnight reliability, Android lint, and signed
  release packaging were not performed. Relay code was reviewed, not changed or
  revalidated in this follow-up. No hosting resources were purchased or deployed.

## Device acceptance still required

1. Update both clients, scan pinned-WSS QR, and confirm notifications and ACKs.
2. Install an app, reconnect, and confirm its icon/category appear.
3. Uninstall that app, reconnect, and confirm it disappears without losing other
   app rules. Reinstall and check the intended rule restoration behavior.
4. Disconnect manually while a reconnect attempt is running. Neither client should
   resume delivery until an explicit reconnect action.
5. Run screen-off and Doze tests, then change Wi-Fi/hotspot and verify retry and
   queue recovery. Record failures, not just successful runs.
6. Test stale socket replacement and ensure old messages cannot update current
   diagnostics or reintroduce an older app inventory.

Do not overwrite production artifacts or call this a production release based
only on unit tests and compilation. See the original September security audit
for additional app-lock, signing, replay-protection, and lifecycle blockers.
