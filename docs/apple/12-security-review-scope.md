# Current security review and pre-edit scope

Date: 2026-09-07 (America/Chicago). Reviewed HEAD:
`cdf418f2f8f2d07700d137a563eef97ce1c92aa0`.

**Verdict: changes requested for the two client lifecycle findings below.**
This is a scoped source review, not a whole-app security certification. No
application code, dependencies, database, deployment, or Git history was changed.
Only this document was added by this review. Findings concern the current HEAD,
not an earlier assistant checkpoint or the claims in the Apple studies.

The initial tracked modifications were `README.md` and `docs/PROJECT_MEMORY.md`;
`docs/BLUEPRINT.md` and `docs/apple/` were untracked. They belong to the user.
Concurrent parent work, including documents 09 and 13, is preserved. Source line
references below are repository-relative and refer to the HEAD above; the
reviewed application sources have no working-tree diff against that HEAD.

## Findings

### F1 - High: enrollment fallback bypasses the saved phone pin with old QR secrets

Primary locations: `desktop/src-tauri/src/sync/relay_client.rs:177-182`,
`:214-227`, `:249-258`, and `:278-288`.

The shared channel correctly rejects a mismatched phone identity. The desktop
caller then changes the policy for the next connection: any error containing
`handshake rejected`, while a pairing code is live, arms enrollment. The next
session uses `Session::desktop_enrollment`, not the saved phone pin, and saves
the newly learned key without a separate local approval of that replacement.

Evidence chain:

- `shared/secure-channel/src/lib.rs:272-280` rejects a learned static key that
  differs from the expected pin. Lines 130-137 explicitly prohibit using the
  enrollment constructor as a fallback for a mismatched saved pin.
- `desktop/src-tauri/src/sync/relay_client.rs:177-182` arms enrollment after that
  failure. Lines 226-227 use an error-string match, not an authenticated request
  carrying a fresh invitation. The local test at lines 549-558 explicitly expects
  this rearming behavior.
- `desktop/src-tauri/src/commands/pairing_cmd.rs:131-164` reuses a live LAN pairing
  key or creates a new five-minute one. Lines 247-270 always reuse the current
  relay pair's phone capability and Noise PSK; regenerating a QR does not rotate
  those credentials. `sync/relay_identity.rs:63-68` creates the PSK separately,
  and `:106-116` permits overwriting the existing phone pin while retaining it.
- `desktop/src-tauri/src/state.rs:148-157` stores enrollment permission as a
  boolean, with no pair ID or expiry. `pairing_code_is_live` at lines 240-247
  checks a saved invitation's time, not whether the UI is actually displaying it.
  `mark_manual_disconnect` at lines 321-339 clears the LAN code but does not
  disarm enrollment. `run_session` does not recheck invitation expiry or pause
  before saving the pin. An armed invitation can therefore be used after its
  displayed lifetime or after disconnect, until consumed or the process ends.
- Pin persistence at `sync/relay_client.rs:283` precedes both the confirmation
  exchange and the application AUTH gate reached through `bridge` at line 305.

Concrete scenario: desktop D has pinned phone A. A holder of an earlier QR for
the still-current relay pair has its capability, PSK and D's public key, but not
A's private key. While a later pairing code is live, the holder connects using
a fresh phone identity B. The pinned handshake fails and arms enrollment. On
retry B completes the PSK-authenticated handshake, and D persists B in place of
A. No newly scanned LAN pairing key is checked before this trust mutation.
A holder can also use an already armed enrollment after the five-minute window.

Confirmed impact: unauthorized replacement of the saved Noise identity and
loss of A's pinned relay authentication. This defeats the extra protection that
a saved static pin should provide after QR-secret exposure. It does **not** let
an attacker without the PSK complete Noise; a relay capability alone is not
enough. An untrusted relay alone cannot decrypt or impersonate a pinned peer.

Application-access limit: `server/ws_server.rs:587-624` separately checks the
current or a saved application pairing key, and `:230-281` applies pause and
reconnect policy. Without such credentials B still fails application AUTH, but
the pin has already been replaced. If B also has an accepted saved pairing key
and automatic reconnect is allowed, that second gate does not restore the
missing Noise identity check. No live notification disclosure was demonstrated.

Proposed fix: retain the saved-pin requirement on every ordinary reconnect;
never translate authentication failure into enrollment permission. Model an
explicit, pair-bound, expiring, single-use replacement invitation, with fresh
authorization material distinct from credentials in an old QR. Recheck and
consume that invitation atomically with pin persistence. Cancel it on expiry,
disconnect and pair change. Do not silently rotate the long-lived device key.

### F2 - Medium: deleting a saved device leaves its relay capability usable for eviction

Primary location: `desktop/src-tauri/src/commands/pairing_cmd.rs:305-329`.

Deleting a device sends a cooperative `UNPAIR` message if it is active and
deletes its local `paired_devices` row. It does not revoke the relay pair, clear
the relay capability, or retire its Noise pairing secrets. A hostile former
phone can ignore `UNPAIR` and continue authenticating to the relay.

Evidence chain:

- `desktop/src-tauri/src/db/store.rs:472-478` deletes only the local device row.
  `commands/pairing_cmd.rs:305-329` contains no relay revoke operation.
- The actual revoke flow is separate: `commands/relay_cmd.rs:109-124` calls
  `sync/relay_api.rs:129-149`, then forgets the Noise secret and current pair.
  Device deletion does not call this flow.
- `relay-worker/src/account.ts:5` sets the pair lifetime to 30 days; lines 91-94
  provision both role credentials for that lifetime. Socket authentication at
  lines 164-169 depends on those records, not the desktop's saved-device table.
- At `relay-worker/src/account.ts:173-174`, an authenticated same-role replacement
  or join against an established generation calls `retire`. Lines 129-132 fence
  and close both sockets. This happens before any device Noise handshake.
- A subsequent desktop QR reuses this pair and phone capability at
  `commands/pairing_cmd.rs:247-270`, so pairing a replacement phone does not
  necessarily remove the former phone's routing access.

Concrete scenario: delete a previously paired phone while its relay pair remains
valid, then pair a replacement using the same configured relay pair. The former
phone retains the phone-role bearer capability. Connecting that role retires
the replacement's and desktop's sockets without needing the replacement's Noise
private key. Repeating joins can interrupt relay availability until the pair is
revoked or expires. The socket ceiling limits accumulation, not this retained
authorization. This is a missing client revocation step, not a claim that the
Worker accepts invalid credentials.

Confirmed impact is relay availability and retained routing authority after
device deletion, not bypass of application AUTH or recovery of plaintext. An
ordinary temporary disconnect need not revoke pairing; permanent deletion must
have explicit, effective revocation semantics.

Proposed fix: associate saved devices with their relay-pair ownership, revoke
the affected pair on permanent removal, and require fresh credentials for the
replacement. Make failed/offline revocation visible and keep a retryable pending
state. Do not report server revocation as successful merely because local rows
were deleted. The current single relay-pair model means shared-pair disruption
must be resolved explicitly before implementing this change.

## Actual authentication and key paths

| Boundary | Current source evidence |
|---|---|
| Relay owner operations | `relay-worker/src/index.ts:26-39` verifies a Firebase token and derives account routing from project and UID. `firebase.ts:55-80` checks RS256, issuer, exact audience, time claims, subject, verified email and excludes tenant tokens. |
| Relay socket access | `index.ts:16-24` and `account.ts:151-202` require role-specific bearer capabilities in the Authorization header. Account/pair IDs in the path are routing metadata, not bearer secrets. |
| Credential creation | `http.ts:53-63` generates independent random 32-byte capabilities and context-bound hashes; `account.ts:70-94` stores hashes and returns credentials only on creation. |
| Desktop owner token | `desktop/src/lib/firebaseAuth.ts`, `firebaseRelayToken`, refreshes the verified user's token. `sync/relay_api.rs:77-126` sends it in a bearer header to provision the pair; saved pair data does not include that ID token. |
| Desktop device secrets | `sync/relay_identity.rs:23-46` persists a generated Noise identity; `:63-96` manages the separate PSK and phone pin. `db/store.rs:312-330` uses the encrypted settings store. `db/encrypted.rs:64-74,259-276` requires keyed access. |
| Desktop storage root | `security/database_key.rs:35-48,220-265` selects platform storage and uses user-scoped Windows DPAPI. The macOS Keychain branch exists at `:51-114`; it was not exercised on a Mac. |
| Phone enrollment input | `android/app/src/main/java/com/focusbridge/android/pairing/PairingManager.kt:124-146` stores the QR desktop public key and PSK separately from the relay capability and application pairing key. |
| Phone private identity | `security/DeviceIdentityStore.kt:30-80` wraps a random static private key with a separate AndroidKeyStore AES key in no-backup storage. This is a wrapped exportable Noise key, not a hardware-resident X25519 operation. |
| Phone pairing storage | `di/DatabaseModule.kt:36-55` uses the encrypted Room factory; `security/DatabaseKeyStore.kt:31-63,92-118` obtains and wraps its database passphrase. Android paths in these rows share the prefix above. |
| Live phone handshake | Android `sync/WebSocketClient.kt:187-196,328-343` uses the capability only in the relay header, then passes private identity, PSK, pair ID and pinned desktop key into the native session. `:375-390` sends application AUTH only after Noise confirmation. |
| Native boundary | `shared/secure-channel-jni/src/bridge.rs` validates array lengths, consumes/wipes secret inputs, and retires handles on errors; `registry.rs` serializes ownership and limits sessions. The Kotlin `PhoneSecureSession.kt:55-95` requires confirmation and checks native liveness. |
| Shared channel | `shared/secure-channel/src/lib.rs:114-179,253-365` binds the pair in the prologue, checks learned static pins, requires enrollment approval and transcript-bound confirmations. `:204-234,367-441` fails closed and bounds sessions/records; `records.rs:18-62` enforces ordered, bounded, expiring assembly. |
| Application AUTH | Desktop relay records go through pinned loopback TLS (`sync/relay_client.rs:348,369-377,434-524`) into the same local AUTH handler (`server/ws_server.rs:193-216,587-624`). Noise identity and application pairing identity are distinct gates. |

The older `05-shared-core-and-protocol.md` section 5 incorrectly describes
socket authorization as header-free capability-in-path. Do not implement an
Apple client from that description: it must send `Authorization: Bearer ...`.
This correction is recorded here without modifying that user-owned document.

## Verification and coverage limits

- Read all four `relay-worker/src` files and both production
  `shared/secure-channel/src` files; traced the named desktop and Android client,
  enrollment, storage, and application-auth boundaries. Supporting storage and
  server files were inspected selectively, not certified line by line.
- `npm.cmd run typecheck` in `relay-worker`: passed, exit 0.
- `npm.cmd test` in `relay-worker`: 3 files, 65 tests passed, exit 0, reported
  duration 23.26 seconds. Wrangler also emitted EPERM log-file and export-analysis
  access warnings under the sandbox; the run is not represented as warning-free.
  These tests exercise a local Worker runtime, not the deployed service.
- Worker tests use synthetic certificate/token fixtures and a local outbound
  certificate responder (`vitest.config.ts`, `test/helpers.ts`). This is useful
  boundary testing, not proof of production Firebase configuration or revocation.
  Existing socket/revocation tests do not exercise desktop device deletion or
  its enrollment fallback, so their passing result does not refute F1/F2.
- `cargo test --offline --locked --manifest-path shared/secure-channel/Cargo.toml
  --target-dir "$env:TEMP/focusbridge-security-review-target"`: blocked before
  tests because `aead v0.5.2` was unavailable in the offline cache. No dependency
  download or lockfile change was attempted. No Rust test pass is claimed.
- Shared-channel test cases include rejection of wrong saved pins, replay,
  confirmation misuse and oversized frames. Their existence is not evidence
  that the current client integration enforces invitation semantics.
- F1/F2 are source-traced state-transition findings; no custom exploit code was
  added and no real device, credential, production endpoint or user database was
  used to reproduce them. Required regression reproductions are listed below.
- Not covered: a full LAN-server/parser audit, all Tauri commands and WebView
  permissions, all Android intents/permissions, UI/account-lock bypasses,
  dependency-advisory audit, cryptographic primitive implementation review,
  timing analysis, fuzzing, binary/source correspondence for bundled JNI `.so`
  files, deployment secrets/configuration, Firebase project rules, Cloudflare
  account controls, backup/recovery forensics, macOS/iOS hardware or signing.
- No new concrete flaw was established inside the Worker authorization logic or
  shared Noise primitive wrapper in this pass. That is a bounded result, not
  proof of absence. No unsupported claim of plaintext relay access, arbitrary
  account takeover, or a production secret leak is made.

## Proposed change sequence and rollback gates

**Documentation only now. This plan does not authorize implementation.** Follow
the parent `13-change-record.md`; do not edit or overwrite its existing entries.

1. Before any future edit, recheck HEAD and dirty paths. Capture a protected
   baseline of only the exact files to be changed, including pre-existing local
   edits. Add a per-fix pre-edit record with affected paths, original behavior,
   regression tests, migration decisions and the intended inverse patch. Do not
   use `git reset`, `git clean`, or restore the whole Apple folder.
2. Reproduce F1 first with synthetic device keys in an isolated test database:
   pin A, retain the old QR secrets, use B, trigger rejection while a code is
   live, reconnect and inspect the pin. Assert it stays A. Also test expiry,
   manual disconnect, process restart, pair switching, replay of a consumed
   invitation, and persistence failure before confirmation. Confirm separately
   that application AUTH failure cannot leave an unauthorized pin replacement.
3. Limit the first F1 containment patch to
   `desktop/src-tauri/src/sync/relay_client.rs`,
   `desktop/src-tauri/src/state.rs`, and focused regression tests: remove
   rejection-triggered rearming and require an explicit live enrollment grant.
   A full fresh-secret invitation design will additionally touch
   `commands/pairing_cmd.rs` and `sync/relay_identity.rs`, and may require Android
   pairing/session changes. Decide compatibility and storage layout before that
   second patch; do not silently reset existing pairings or edit Noise itself to
   accept weaker authentication.
4. Reproduce F2 across the deletion command and local Worker runtime: save a
   phone, delete it, then attempt a retained-capability upgrade. After the fix,
   assert denial and inability to evict a replacement. Test offline deletion,
   revoked/expired credentials, duplicate deletion, and ambiguous shared-pair
   ownership. Expected code scope is `commands/pairing_cmd.rs`,
   `commands/relay_cmd.rs`, `sync/relay_api.rs`, pairing metadata storage and the
   calling UI. Final exact files depend on the approved ownership model; no
   schema change should begin before that scope is recorded.
5. Keep patches independent and avoid destructive migrations. For each completed
   patch save its exact forward and inverse diff, excluding user-owned edits;
   verify reverse applicability with `git apply --reverse --check <patch>` on
   the matching worktree before proposing rollback. Revert only those hunks,
   never a previous assistant's entire checkpoint. Do not commit in this task.
6. Code rollback and credential rollback are different: a revoked relay
   capability cannot be restored safely by reverting source. Do not perform live
   revocation, rotate credentials or delete pairing records during development.
   Use disposable fixtures. Before a production rollout, document that recovery
   from revocation is fresh provisioning and explicit re-pairing, not restoration
   of compromised credentials. Obtain approval for that operational effect.
7. Before declaring fixes complete, run Worker tests/typecheck, shared Rust and
   JNI tests, desktop integration tests and Android handshake tests, followed by
   two-device LAN/relay enrollment, reconnect and removal checks. Record any
   unavailable platform as unverified rather than silently widening coverage.

Rollback for this documentation step alone: remove only this newly added file
if it remains unchanged by others. If it has since been edited, reverse only
this review's contribution. Preserve documents 09 and 13 and all other dirty
documentation. No application rollback is necessary because none was edited.
