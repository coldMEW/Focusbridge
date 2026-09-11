# Budget, privacy and acceptance corrections

Independent review, 2026-09-07. Proposed gates; not legal advice or acceptance.

## Free-first without misleading promises

The original distribution study incorrectly generalizes Personal Team
provisioning limits to local Mac applications. Apple's Personal Team limitations
describe registered App IDs/devices/profiles; do not turn these into a universal
seven-day expiry for ordinary locally built Mac executables.
[Apple account overview](https://developer.apple.com/help/account/basics/about-your-developer-account)

Separate four budgets: developer membership, access to hardware, infrastructure,
and maintenance. Membership alone does not buy a Mac, iPhone, signing approval,
unlimited relay quota, radio hardware, or entitlement eligibility. Never promise
that paying a fee resolves the accessory's platform restrictions.

Begin with borrowed/owned hardware and a free developer account where supported.
Use public-repository CI only after checking its current included runner policy;
CI is not a replacement for physical Bluetooth, Keychain prompts, sleep and
permission testing. Do not purchase a Mac rental or enroll in a paid service
without explicit permission.

Apple offers membership fee waivers to qualifying organizations, including
accredited educational institutions. Being an individual student does not
automatically qualify; a university-owned project requires its authorization
and decisions about ownership and signing-key custody.
[Apple fee waivers](https://developer.apple.com/help/account/membership/fee-waivers)

## Privacy is a data-flow audit, not an encryption slogan

Before filling App Store labels, inventory:

| Boundary | Questions to answer from actual configuration |
|---|---|
| Firebase authentication | Email, identifiers, session persistence, deletion and SDK telemetry |
| Relay | IP processing, hashed account identifiers, pair metadata, provider logs and retention |
| Desktop/phone | Message history, masks, caches, database exports, backups and crash reports |
| Accessory | Plaintext exposure, destination keys, firmware logs and factory reset |
| Support | Are screenshots or diagnostic bundles redacted before export? |

Apple's disclosure rules cover relevant app and third-party collection. Evaluate
each category and exception; neither a hashed identifier nor encrypted payload
automatically means no data collection. Ensure marketing, labels, manifests and
the privacy policy agree with observed traffic.
[Apple privacy details](https://developer.apple.com/app-store/app-privacy-details/)

Do not invent export classification from the presence of Rust, Noise or a
third-party crypto library. Inventory the algorithms and uses, then follow
Apple's current questions and the applicable jurisdiction's requirements.
An annual report is not automatically required for every app with E2E encryption.
Account-holder review may be needed before submission.
[Apple encryption compliance](https://developer.apple.com/documentation/security/complying-with-encryption-export-regulations)

## Hardware evidence required

Record exact device models, OS builds, radio adapters, app build hash, source
adapter, account region when relevant, permissions and transport. Avoid storing
real notification bodies in the test record.

| Gate | Required evidence | Fail condition |
|---|---|---|
| Mac receiver build | Apple-silicon build; Intel build if advertised; dependency/linker checks | Platform stub, missing symbol, incompatible minimum OS |
| New installation | Signed/notarized artifact on clean account; offline verification where applicable | Requires disabling Gatekeeper or bypassing security |
| Storage | New DB, upgrade, wrong key, denied Keychain, interrupted migration, backup/restore | Data loss, plaintext residue claim without proof, silent recreation |
| Connectivity | LAN/hotspot, isolated Wi-Fi, cellular relay, IPv6, network change, captive portal | False connected state or silent downgrade |
| Notification correctness | Add/update/remove, burst, group summary, empty/hidden preview, stale action | Duplicate alerts or fabricated missing content |
| Privacy | Masked native toast, locked UI, app switcher, search, logs, exports | Hidden content exposed outside the main card |
| App controls | Apply/reject/persist rules; unsupported features labeled | UI says applied but source cannot enforce |
| Background | Locked overnight, Low Power Mode, sleep/wake, reboot, forced quit | Marketing promises delivery outside OS guarantees |
| Device identity | Reinstall, key rotation, revocation, old QR, concurrent reconnect | Name/IP treated as authentication |
| Account lifecycle | Guest, login, logout, deletion, token expiry, local lock recovery | Auth provider failure disables offline LAN unexpectedly |
| Actions | Source-authorized action labels and expiry; repeated requests | Action targets changed notification or executes twice |

Use fault injection and simulator tests for deterministic logic, but call them
simulator tests. Do not label those results an iPhone endurance test. Set target
latency percentiles and battery budgets before measuring; do not choose them
after seeing results. Record genuine unsupported states rather than hiding them.

## Questions needed before the first Apple spike

1. Which owned or borrowed Mac and iPhone can be used, and their OS versions?
2. Is the first iPhone goal nearby Windows notifications, nearby Mac notifications,
   or direct Internet forwarding? These have different feasibility gates.
3. Is a small hardware bridge acceptable if public desktop ANCS access fails?
4. Is an eligible EU customer deployment relevant? Do not infer geography from
   the development PC's timezone, and do not recommend region spoofing.
5. Who owns any developer membership and long-lived release keys?

No application code changes are authorized by the conclusions of this document
alone. The next engineering change should have a reviewed bounded design and a
pre-recorded rollback plan, preserving the currently working Android/Windows path.
