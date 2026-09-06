# Release verification

Verdict: NOT READY TO SHIP. No commit or push performed.

## Checks on the current working tree

| Check | Result |
| --- | --- |
| Relay cargo test, MSVC, offline | 37 passed |
| Desktop workspace cargo test, MSVC, offline | 24 core + 13 integration tests passed |
| Desktop Vitest | 26 passed |
| Android testDebugUnitTest, forced with --rerun-tasks | 46 passed, zero failures/errors |
| Android lint | Task passed; 58 warnings remain |
| Android assembleDebug | Passed/up-to-date; not release signing verification |
| Desktop pnpm build (TypeScript + Vite) | Passed |
| Desktop and relay Clippy, all targets, warnings denied | Passed |
| git diff --check | Passed |
| ADB devices -l | Pixel 7 authorized later in this session; see device checks below |

146 automated tests passed. Placeholder test targets with zero tests are not
counted. Some mobile UI security tests are source guardrails, not device tests.

## Outstanding release gates

- Android release config still selects debug signing.
- Lint warnings: 51 GradleDependency, 3 AndroidGradlePluginVersion, and one each
  UnusedResources, IconXmlAndPng, CustomX509TrustManager, WakelockTimeout.
  A warning is not automatically a vulnerability, but needs review/disposition.
- The physical-device section below supersedes the initial lack of LAN tests.
  Full locked/background/overnight stability and clean-PC installer acceptance
  are still outstanding.
- Secure cross-network relay integration and remaining security findings in
  security-review-2026-09-05.md are unfinished. No public relay is deployed.
- No production installer or signed-for-distribution APK was certified here.
  A later `assembleRelease` check generated an APK with the existing debug
  signing configuration; it is a test artifact, not a shipping release.

Do not commit or push until the user-required full release gate is satisfied.

## Physical device checks

Device: Google Pixel 7, Android 17. Current debug APK installed over the existing
app successfully using `adb install -r`; app data was not reset. Notification
listener and foreground sync services were running. Existing battery exemption
was present. Windows test GUI launched with the actual WSS listener on TCP 9173.
No ADB port reversal/tunnel was used; this was real LAN traffic.

- Initial WSS authentication occurred at 07:43:38 UTC. The phone app inventory
  snapshot contained 108 present apps; absent historical apps remained archived
  rather than exposed as current inventory. This is not yet an install/uninstall
  acceptance test.
- A uniquely tagged synthetic notification reached Windows with the same ID
  as the Android record. Android later recorded SENT after desktop ACK.
- The first burst/Doze attempt overlapped manual network changes and was stopped.
  Forced idle and battery simulation were restored. Those delivery timeouts are
  NOT a valid successful Doze/latency test. The three synthetic MessagingStyle
  messages were subsequently captured as separate rows and marked SENT.
- Reproduced the stale-status bug in source and a failing frontend test: the old
  UI accepted 120-second-old heartbeats; the server waited 180 seconds.
- Initial active-probe fix: phone Wi-Fi disabled at approximately 08:03:54 UTC;
  Windows persisted disconnected after 7.24 seconds and logged heartbeat timeout.
  Wi-Fi was re-enabled; automatic reconnect completed 12.52 seconds later.
- One notification generated while offline arrived exactly once after reconnect.
  A new notification after recovery appeared in Windows in 500 ms measured on
  the host, including ADB/post/poll overhead. Phone and PC clocks differed, so
  cross-device timestamp subtraction was not used for latency.
- The initial fix passed 41 Rust workspace tests, 26 frontend tests, TypeScript/
  Vite build and Clippy. A second review identified batch/Pong starvation and
  deadline-alignment edge cases. Corrections subsequently passed 49 Rust
  workspace tests (24 core, 7 inventory, 18 connection), including a real
  controllable-WebSocket slow-batch regression. After the final Clippy fix,
  all 18 connection tests passed again; Clippy with denied warnings passed.
  UI stale cutoff is 12 seconds to allow probe scheduling/snapshot slack;
  server probes every 3 seconds with a 6-second response deadline. These final
  corrections still need physical-device acceptance.
- Windows was stopped before encrypted-storage fixture work. No unverified
  storage migration was run on the user's real database or phone.

Still unverified: full six-minute/overnight screen-off stability, real email and
social-app parsing, actual Windows toast visibility/identity under DND, UI pixel
assertions for every status location, TLS interception attack tests, and any
different-network relay session. The public relay is not deployed.

## Account setup and auth UI follow-up

- Added desktop password-eye controls; fresh Vitest run passed 41 tests,
  TypeScript passed, and the Vite build passed. The first sandboxed Vitest attempt failed because esbuild
  could not read a parent directory; the approved rerun passed without changing
  the code to work around sandbox permissions.
- Installed pinned Wrangler 4.129.0 in `tools/cloudflare`. Frozen/offline install
  passed; npm advisory audit returned zero known vulnerabilities for this
  tooling's lockfile. This does not certify the app or relay.
- Browser OAuth completed. `whoami` matched the owner-supplied Cloudflare account
  and confirmed encrypted credential storage backed by Windows Credential
  Manager. Permissions are account/user read plus offline refresh only.
- The missing-write-scope warning is expected for this verification-only login.
  No Worker, Durable Object, DNS record, paid plan, or deployment was created.
  Free-plan status still needs dashboard confirmation before deployment.
- Git ignore verification passed for `tools/cloudflare/.env.local`. Account
  metadata is not compiled into either client. No token value was printed.
- Encrypted database migration source is being staged separately. Runtime
  activation remains gated pending isolated native migration tests and review;
  no live database was migrated. Cloud traffic remains blocked.
  Desktop `encrypted_database.rs` is deliberately gated and executes zero tests
  at this checkpoint; it is NOT included in the 49 passing Rust tests.

## Android storage staging checks

- Fresh JVM XML reports: 61 tests, zero failures/errors/skips (15 new storage
  key/state tests). These do not execute native SQLCipher or Android Keystore.
- Worker-reported Gradle gates: `assembleDebugAndroidTest`, `assembleRelease`,
  and `lintDebug` passed. Native test APK contains 14 tests, compiled but NOT run.
- Lint: zero errors, 60 warnings, including two added version-catalog warnings.
  Native packaging also warned that `libsqlcipher.so` remained unstripped.
- The production Room provider is unchanged and still plaintext. The encrypted
  builder is not called. No installation or migration touched real user data.
- Remaining: disposable-emulator native tests, real process-death/storage-fault
  recovery tests, safety review, then deliberate runtime activation and device
  acceptance. No release is approved by these build results.
