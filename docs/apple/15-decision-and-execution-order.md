# Apple expansion: reviewed decision and execution order

Updated 2026-09-08. Source baseline: `cdf418f`.
Status: research handoff, not implemented or hardware-verified Apple support.

## Read the corrected reports first

| Report | Purpose |
|---|---|
| [18: implementation leads](18-open-source-reproduction-leads.md) | Windows/Linux ANCS references, privileged-device distinction and reproduction gates |
| [19: verified routes and architecture](19-verified-capture-routes-and-architecture.md) | **Read first.** Primary-source verification of every capture route, the revised architecture, and the two experiments that decide the product |
| [20: iOS build blueprint](20-ios-app-build-blueprint.md) | The concrete iPhone app build plan: stack, module map, protocol conformance, platform traps, desktop changes, test and work order |
| [09: feasibility](09-independent-feasibility-review.md) | Corrects blanket impossibility claims; separates Bluetooth, hardware and EU paths |
| [10: accessory framework](10-accessory-framework-verification.md) | Live Apple API, eligibility, extension and policy checks |
| [11: macOS port](11-macos-port-verification.md) | Actual code gaps and platform-specific engineering/test plan |
| [12: security](12-security-review-scope.md) | Two source-traced lifecycle findings, coverage limits and pre-edit fix plans |
| [13: change record](13-change-record.md) | Baseline preservation and rollback scope |
| [14: budget and acceptance](14-budget-privacy-and-test-gates.md) | Free-first choices, privacy corrections and physical-device gates |

Original reports 00-08 remain historical input, not an independently verified
verdict. In particular do not repeat their assertions that all non-EU iPhone
notification forwarding is impossible, that the Mac port loses no features,
or that encrypted relay routing establishes full policy compliance.

## First: resolve the current security findings

F1 concerns the integration policy, not a broken Noise primitive. A saved-phone
identity mismatch can arm enrollment on the following connection when a pairing
code is live. Old QR secrets can therefore become authority to replace a saved
pin. The separate application AUTH gate limits impact but does not undo an
already changed pin. Reproduce with synthetic identities before changing it.

Recommended design for approval: ordinary reconnect always requires the saved
pin. Replacement is a separate explicit user action with a fresh, pair-bound,
expiring, single-use invitation. Authentication failure never creates authority.
Validate invitation and persist the authorized identity in a coordinated commit;
cancel pending authority on disconnect, pair change and expiry. Test persistence
failure and concurrent connections. Existing phones must retain their identity.

F2 concerns permanent removal. Deleting the local saved-device row leaves the
former phone's relay capability valid. It can still interrupt relay availability
through same-role connections even without defeating the new peer's Noise key.

Recommended design for approval: permanent removal revokes the associated relay
pair, distinguishes local removal from confirmed remote revocation, and retains
a retryable revocation task if offline. New enrollment gets fresh credentials.
Temporary disconnect remains distinct. Because the current model shares a relay
pair, the UI must explain any required re-pairing; do not revoke an unrelated
device's credentials by guessing the association.

Both fixes need integration regressions, not just a green crypto test suite.
No source fix has been applied in this research step. See report 12 for exact
paths, affected tests, migration decisions and rollback requirements.

## Second: port Android-to-Mac reception

Keep this independent of iPhone capture. Reuse the Rust/Tauri/React receiver and
existing shared Noise engine; do not create an unnecessary Swift networking
rewrite. Replace the non-Windows `hostname -I` fallback with native interface
enumeration suitable for macOS. Exercise the existing Keychain implementation
rather than assuming it is absent.

Required milestones: linked native build, permission-aware LAN discovery,
signed-app notifications, Keychain/SQLCipher recovery, sleep/wake reconnect,
tray/Dock lifecycle, Mac-specific Settings, installer and notarization checks.
Current cargo-check-only Mac CI cannot establish these outcomes. Each milestone
needs a recorded test result on the OS/architecture actually advertised.

## Third: settle iPhone source feasibility with small prototypes

Run separately, rather than building an entire iOS application first:

1. Ordinary Windows GATT access to an explicitly paired stock iPhone. Record
   service access, permissions, events and reconnect behavior. Phone Link's
   existence does not prove the same access for a third-party implementation.
2. Native macOS CoreBluetooth access. Record denial honestly; do not infer Mac
   access from Windows behavior or an external BLE board.
3. EU accessory-framework entitlement and destination eligibility check,
   followed by a signed minimal forwarding extension. Picker success alone is
   not approval of a general-purpose PC accessory or production eligibility.
4. Optional owned BLE-board prototype if software access fails. Treat it as a
   separate hardware product and plaintext trust boundary, not a free bypass.

Select the product scope from these results. No route establishes unrestricted
worldwide iPhone capture and Internet forwarding with Android-equivalent controls.
Do not invent missing notification content, installed-app inventories, background
guarantees or notification actions to make the UI look complete.

## Needed from the owner

Before running Apple proofs, identify available Mac and iPhone models/OS versions.
Before selecting EU deployment, establish genuine customer eligibility and team
capability access. Before buying hardware or membership, obtain explicit budget
approval. No passwords, private keys, account recovery codes or session tokens
should be sent in chat.

## Review completeness

This is a substantial component-level research set, not a review of every line
of the repository. The security report lists unexamined areas. No new exploit
reproduction, Mac/iPhone build, physical-device experiment, or store submission
was run during this documentation continuation. Those are remaining work, not
implied successes. App code and user data were left unchanged.
