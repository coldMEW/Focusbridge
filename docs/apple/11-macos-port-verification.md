# macOS port verification: current code, platform evidence, and remaining gates

Date: 2026-09-07. Research method: inline source inspection and official platform
documentation. Repository HEAD inspected:
`cdf418f2f8f2d07700d137a563eef97ce1c92aa0`.

**This is a source-level assessment, not a successful Mac build or release
verification.** No macOS executable, hardware session, signing identity, Keychain
interaction, or Bluetooth connection was tested in this investigation. No app
code, dependency, existing document, or commit was changed by this report.

The inputs were [02: desktop port](02-macos-desktop-port.md),
[04: Mac as source](04-macos-as-source.md),
[05: shared core](05-shared-core-and-protocol.md), and
[07: roadmap](07-roadmap.md), followed by actual `desktop/`, `shared/`, and
desktop CI code. Their proposed designs are not evidence of implementation.
The separate [09: independent feasibility review](09-independent-feasibility-review.md)
owns the wider Windows and hardware-bridge investigation. Cost and broader
privacy-policy corrections are outside this report's scope.

## 1. Decision and prioritized findings

**Android to macOS is a credible port of the existing desktop receiver, but it
is not verified and is not merely a packaging switch.** It does not depend on
an iPhone source, ANCS, a Swift wrapper, or an Apple accessory experiment.
Keep those investigations off its critical path.

| Priority | Finding from current source | Consequence / next gate |
|---|---|---|
| P0: release gate | A macOS Keychain implementation already exists in `security/database_key.rs:46`; doc 02 describes it as future work. | Validate and harden the existing implementation, not a second replacement. Check signed-app access, key loss, path changes, and background behavior. |
| P0: functional gap | `commands/pairing_cmd.rs:80` uses `hostname -I` on every non-Windows OS. Apple's hostname utility does not implement that option. | Implement native Mac interface enumeration later; test offline LAN, VPN, hotspot, Ethernet, and multiple adapters. A successful default-route probe can conceal the defect. [Apple hostname source](https://raw.githubusercontent.com/apple-oss-distributions/shell_cmds/main/hostname/hostname.1) |
| P0: evidence gap | The Mac CI leg ends at `cargo check --locked`; no Rust tests, linked app, DMG, signing, or device test is performed by that workflow. | A green matrix entry is not a working Mac product. Record actual native build and runtime results before calling the port supported. |
| P1: native behavior | Notification plugin initialization and calls already exist, but the pinned plugin cannot establish actual desktop permission or delivery status. | Verify the bundled app and add truthful diagnostics/authorization handling as needed; see section 5. |
| P1: storage risk | Unix migration publication uses hard links and directory syncing, with an obsolete comment claiming non-Windows production keys are rejected. | Exercise that path on macOS/APFS, including interruption and recovery. It is production-reachable with the existing Keychain branch. |
| P1: UX gap | Window close is intercepted on all platforms; Settings exposes Windows firewall setup; no login-item implementation was found. | Define and verify Mac lifecycle, onboarding, and login behavior rather than assuming Tauri supplies the desired product behavior. |
| P1: architectural correction | Local-network privacy must be assessed per socket operation, not from the presence of a listener. | Re-evaluate doc 02's permission assumptions using section 4; do not redesign loopback based on an untested premise. |
| Separate feasibility gate | Historical Apple staff evidence explicitly rules out ANCS consumption through CoreBluetooth on OS X 10.11. No current reversal or working FocusBridge Mac client was established. | Do not promise an iPhone-to-Mac Bluetooth feature or infer it from Windows Phone Link. |

Priority here is order of verification work, not a claim of a reproduced Mac
failure or a newly completed security audit.

## 2. What is actually shared

The desktop workspace is [desktop/Cargo.toml](../../desktop/Cargo.toml): members
`src-tauri` and `core`. The application depends directly on
`focusbridge-core` and `../../shared/secure-channel`; macOS does not need JNI or
an xcframework to use them.

```text
Android LAN WSS
  -> desktop/src-tauri/src/server/ws_server.rs
  -> core handler / authenticated envelope handling
  -> SQLCipher store
  -> vault-gated frontend event and native notification

Android relay ciphertext
  -> public relay WSS -> sync/relay_client.rs
  -> shared/secure-channel session and record handling
  -> pinned WSS connection to 127.0.0.1:9173
  -> the same ws_server application path
```

Evidence: `src-tauri/src/lib.rs:44` initializes plugins, database, certificate,
tray, LAN listener, and relay supervisor; `server/ws_server.rs:503` applies
stored work; `sync/relay_client.rs:342` bridges and `:434` connects locally.
All abbreviated desktop paths below are relative to `desktop/` unless stated
otherwise. Line references are navigation aids for the inspected snapshot;
named functions should be used if subsequent edits move them.

The shared Noise profile is
`Noise_XXpsk3_25519_ChaChaPoly_SHA256`, declared in
[shared/secure-channel/src/lib.rs](../../shared/secure-channel/src/lib.rs#L14).
Its authenticated enrollment, confirmations, record limits, and replay handling
are existing implementation, not a Swift reimplementation requirement.
[shared/secure-channel-jni](../../shared/secure-channel-jni/Cargo.toml) is the
Android wrapper. No `shared/secure-channel-ffi` implementation was present in the
inspected tree; doc 05's C ABI and xcframework are proposals for a different
integration, not prerequisites for this Tauri receiver.

Doc 05's `platform` / `capabilities` AUTH example is also a proposal, not an
implemented negotiation contract. The current
[protocol enum and envelope](../../desktop/core/src/protocol.rs) use JSON
payloads; [shared/protocol.json](../../shared/protocol.json) and current handler
paths must remain compatible with Android. Do not advertise iPhone action,
inventory, or source capabilities simply because a generic message enum exists.

## 3. Component matrix

Statuses distinguish **existing code**, **portable candidate**, **missing
platform work**, and **unverified native behavior**. None means Mac-tested.

| Component | Actual code path / observed behavior | macOS disposition and remaining verification |
|---|---|---|
| Runtime and frontend | `src-tauri/src/main.rs:1`, `src-tauri/src/lib.rs:44`, `src/main.tsx`, `src/App.tsx`; Tauri 2, React 18, Vite. Windows subsystem and app identity code are cfg-gated. | Existing reusable application. Tauri uses WKWebView on Mac, not WebView2. Test real assets, fonts, scrolling, keyboard, IPC, and restart behavior on each supported OS. [Tauri Webview Versions](https://v2.tauri.app/reference/webview-versions/) |
| Dependency baseline | `src-tauri/Cargo.toml`; `desktop/Cargo.lock` pins Tauri 2.10.3, notification plugin 2.4.0, notify-rust 4.16.1, mac-notification-sys 0.6.12, and direct security-framework 2.11.1. | Build these versions first. Current upstream plugin documentation is not proof of the locked version's behavior. Do not silently update dependencies during verification. |
| Toolchain | App manifest declares Rust 1.75, but `shared/secure-channel/Cargo.toml` requires 1.85; `src-tauri/rust-toolchain.toml` selects floating stable. | 1.75 is not the effective minimum for the graph. Record actual rustc/Cargo/Xcode/SDK versions and validate the locked dependency graph. Pin a tested toolchain in future CI work. |
| LAN listener | `src-tauri/src/lib.rs:67` binds `0.0.0.0:9173`; `server/ws_server.rs:39` creates a Tokio listener and rustls acceptor; plaintext is rejected at `:98`. | Portable candidate. Verify bind failure, occupied port, firewall state, TLS handshake, wrong pin, authentication, and reconnect. Listener failure is currently logged by `start`, not represented as a dedicated diagnostics field. |
| Address candidates | `commands/pairing_cmd.rs:30`, `:80`, `:100`: UDP route probe to `8.8.8.8:80`, then command output, then loopback fallback. | Concrete port work. `hostname -I` is unsuitable on stock macOS; no subprocess exit-status check is made. If the route probe fails and no candidate is found, the phone receives its own loopback address as a nominal endpoint. Native enumeration and an honest no-LAN result need validation. |
| QR codec | `src-tauri/src/pairing/qr_generator.rs` re-exports `core/src/qr.rs`; `make_qr`, `encode_compact`, `decode_compact`; `commands/pairing_cmd.rs:129`. | Reuse codec and current Android-compatible fields. Test a Mac-rendered code with the unmodified Android scanner, including relay and LAN-only variants. An iPhone scanner experiment is not an Android-to-Mac release dependency. |
| Certificate identity | `pairing/cert_manager.rs:10` persists `certs/desktop-cert.pem` and `desktop-key.pem`; `core/src/cert.rs:11` generates the certificate. | Existing file-based identity, not Keychain-backed TLS private-key storage. Verify restrictive effective file/directory access, corruption/partial-file recovery, persistence, and repinning behavior. Do not describe every secret as protected by the new Keychain branch. |
| TLS provider | `src-tauri/src/lib.rs:30` explicitly installs ring before any connections; `server/tls.rs:5` builds the listener configuration. | Preserve initialization and verify a real handshake on Mac. The unit test in `lib.rs` is not run by the existing check-only CI. |
| Relay HTTPS provisioning | `sync/relay_api.rs:77`, `:130`; reqwest with `rustls-tls`; frontend supplies a freshly refreshed Firebase token. | Portable candidate. Verify normal certificate validation, DNS, timeouts, expired/unverified accounts, enable/disable, and private data redaction. No Worker protocol change is identified as necessary for the Mac receiver. |
| Relay WSS and Noise | `sync/relay_client.rs:96`, `:232`, `:342`; `shared/secure-channel/src/lib.rs`; identity and PSKs stored through `sync/relay_identity.rs`. | Existing reusable transport/security code. The WSS dependency enables `rustls-tls-native-roots`; doc 02's blanket webpki-roots description is not the actual WSS configuration. Exercise public CA validation independently from local pinning. |
| Local relay bridge | `sync/relay_client.rs:434`, `PinnedCertificate` at `:473`: exact DER SHA-256 pin, rejects extra chain certificates, verifies handshake signatures. | Preserve this verifier and IPv4 literal. Verify public relay plus local bridge end-to-end; public WSS connectivity alone cannot prove delivery. No demonstrated need for an in-process replacement. |
| OS database key | `security/database_key.rs:34`, `:46`; direct macOS security-framework dependency. Windows DPAPI and Mac Keychain are already separate cfg branches. | Existing Mac implementation with unverified release behavior. See section 5 for API, identity, ACL, and recovery details. Other unsupported OSes fail closed rather than store a plaintext key. |
| SQLCipher | `db/encrypted.rs:56`, `:124`, `:230`; rusqlite `bundled-sqlcipher-vendored-openssl`; `db/store.rs` uses encrypted connections. | Compile/link native dependencies and verify encryption on the actual Mac bundle. No assumption that system Perl or an installed Homebrew library makes this release-ready. Inspect dylib dependencies and test a clean machine without developer tools. |
| Migration durability | `db/encrypted.rs:643` syncs a parent directory on Unix; `:651` publishes without replacement, using `hard_link` then removal on non-Windows. | Existing path now reachable in Mac production despite its fixture-only comment. Test interrupted migration, orphan links/files, duplicate processes, WAL recovery, wrong key, and recovery-copy preservation on APFS. No crash-durability guarantee inferred from portable fixture tests. |
| Vault and inbox | `commands/notification_cmd.rs:13`, `:22` reject listing while locked; `server/ws_server.rs:525`, `:537`, `:544` store then gate message display. | Reuse security boundary. Test locked startup, idle relock, notification races, background receipt, hide/reopen, sleep/wake, and native banners. Test already-delivered notification previews separately; current gating does not prove their removal on lock. |
| Settings and frontend persistence | `src/App.tsx:85`, `:90` hydrate from Rust; `src/stores/settingsStore.ts` is in-memory Zustand; `SettingsPanel.tsx:119` also writes lock timeout to localStorage. | Do not describe all settings as browser storage. Verify DB-backed preferences and browser-backed state separately across relaunch and upgrade. |
| Account flow | `src/lib/firebaseAuth.ts:1`, `:30`, `:39`, `:66`: email/password, verification/reset mail, token refresh; not a provider popup flow. | Same code is a candidate, not WKWebView validation. Test persistence, verification completed in an external browser, network loss, expired token, and local vault independence. Adding Sign in with Apple is not required merely to compile this receiver. |
| CSP and capabilities | `src-tauri/tauri.conf.json` has a restricted CSP and Google token endpoints; `capabilities/default.json` grants `core:default`. | Validate bundled custom-protocol IPC and account requests. Rust notification calls already work outside a JS notification permission command; if new JS plugin commands are added later, grant only their required Tauri permissions. Tauri ACL, OS consent, and App Sandbox are separate layers. |
| Native alerts | `desktop_notifications.rs:6`, `:30`; plugin initialized in `lib.rs:46`. Current phone alert sends title/body, not a source-app attachment or action. | Existing plugin, not a missing Windows-only toast rewrite. Test authorization, app identity, suppression, content masking, click behavior, and failure reporting. See pinned plugin caveat below. [Tauri Notifications](https://v2.tauri.app/plugin/notification/) |
| Tray/menu | `tray/menu.rs:7` installs Show Window / Quit through `TrayIconBuilder`. | Reusable abstraction. Test menu-bar visibility, icon appearance, hidden-window restore, and explicit Quit. Do not assume the colored Windows icon is ideal for the Mac menu bar. [Tauri System Tray](https://v2.tauri.app/learn/system-tray/) |
| Closing and quitting | `lib.rs:117` intercepts every close, emits `focusbridge://close-requested`, and shows/focuses the window; `src/App.tsx:129`, `:291` presents tray-or-quit UI. | Not automatic Mac-style close-to-background. Decide close, Dock reopen, Command-Q, logout, and minimized/background behavior. Keep explicit Quit distinct from hiding the window. |
| First-run setup | `commands/windows_setup_cmd.rs:10` invokes elevated Windows firewall setup; `:49` returns a no-op message elsewhere. `SettingsPanel.tsx:498` invokes it. | Mac-specific onboarding missing. Current setup does not implement autostart even on Windows. Remove/hide misleading Windows UI in a later app change, not in this documentation task. |
| Launch at login | No autostart dependency, registration, or SMAppService call found in inspected desktop source. | Missing feature, not a renamed command. Choose SMAppService with a compatible minimum OS, or deliberately evaluate another implementation. Tauri's autostart example uses `MacosLauncher::LaunchAgent`; that is not evidence it uses SMAppService. [Apple SMAppService](https://developer.apple.com/documentation/servicemanagement/smappservice), [Tauri Autostart](https://v2.tauri.app/plugin/autostart/) |
| Diagnostics | `commands/diagnostics_cmd.rs:7` reports transport, candidates, pin, heartbeat, and auth/disconnect reasons. | No Mac permission, keychain status, listener-ready state, signing identity, or notification authorization probe. Add observed causes later without turning every timeout into a permission diagnosis. |
| Bundle/distribution | `tauri.conf.json` lists MSI, DMG, AppImage and includes `icon.icns`; no checked-in Mac Info.plist, entitlements, or explicit `bundle.macOS` configuration was found. | Packaging intent exists, not an artifact. Select explicit app/DMG bundles for the spike and inspect generated metadata. No signing, notarization, or clean-install evidence here. |
| CI/test execution | `.github/workflows/desktop-ci.yml:11` includes macos-latest and frontend checks; `:34` runs cargo check. | Existing check coverage only. Add linked native builds, Rust suites, report artifacts, and release-job protections later. Runner label does not establish Intel coverage or native permission behavior. |
| Bluetooth / Mac as source | No ANCS/CoreBluetooth adapter or Notification Center reader found in the inspected app/shared code. | Not part of the existing receiver. Keep section 6's feasibility boundary separate from ordinary Mac notification display. |

## 4. Networking: correct the premise before changing the design

Apple's [TN3179](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy)
documents local-network privacy from macOS 15. Incoming TCP listen/accept does
not require permission; outgoing local TCP and connected local UDP do. Its
local-network definition excludes loopback by inference from broadcast-capable
interfaces, not from a FocusBridge test. It recommends an Apple-issued signing
identity for reliable attribution. It does not establish `/Applications` as a
universal networking requirement. Bonjour declarations apply when using Bonjour;
the iOS multicast entitlement is not required on macOS. First operations may
fail before consent, so retry behavior matters.

Consequently, doc 02's mandatory-listener-prompt model and doc 07's assumed
loopback blockage are not verified constraints. Test the actual paths instead:

| Path | FocusBridge-specific acceptance test |
|---|---|
| Incoming Android LAN | Reach `9173` from the phone with firewall configurations recorded; distinguish listener bind, reachability, TLS, and authentication failures. |
| Route discovery | Confirm no internet route does not destroy otherwise valid LAN pairing; confirm VPN routing does not monopolize candidates. The UDP probe calls connect, not send. |
| Relay | Provision, connect publicly, finish Noise, traverse pinned loopback, persist, ACK, and display after unlock. Repeat under denied local-network state where applicable. |
| Permission UI | Test the signed app from Finder and the intended install location, not only a terminal executable. Preserve unknown status when the underlying failure is ambiguous. |

The [macOS application firewall](https://support.apple.com/guide/mac-help/block-connections-to-your-mac-with-a-firewall-mh34041/mac)
is a separate control. Its settings can permit signed applications or block
incoming traffic; neither exactly two prompts nor a firewall prompt on every
first listen is guaranteed. Do not disable the firewall as an installation step.

## 5. Keychain and notifications need version-specific verification

### Existing Keychain branch

`security/database_key.rs:58` canonicalizes the database's parent and hashes its
full path into the account name. It uses service
`com.focusbridge.desktop.sqlcipher.v1`, `SecKeychain::default()`,
`find_generic_password`, and `add_generic_password`. It requires 32 bytes,
verifies a newly persisted value, refuses a missing database with an existing
key except for recognized migration recovery, and does not fall back to
plaintext or replace an inaccessible existing key.

This is the traditional file-based Keychain API, not an implementation of doc
02's explicit data-protection accessibility/access-control proposal. Apple
distinguishes file-based ACLs from data-protection access groups and recommends
SecItem for new code; selecting the data-protection implementation changes
signing/provisioning considerations. An iOS-style accessibility value cannot
simply be assumed to describe this implementation.
[Apple TN3137](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains)

Required evidence before accepting this design:

- First create, relaunch, and same-identity upgrade preserve the original key and database.
- Keychain denial, lock, deleted item, wrong-length item, and altered signing identity preserve recoverable data and show a useful error.
- Changing the database path does not silently create an empty replacement. The path-derived account is a real migration/recovery constraint.
- Background receipt and login launch work in the intended user session without a per-message access prompt. Startup currently fails if database initialization fails.
- Tests use disposable keychains/accounts, not destructive edits to a user's production login keychain.
- Missing or corrupt certificate PEM files are tested separately; the database key policy does not cover those files.

### Existing notification plugin

FocusBridge uses Rust `NotificationExt`, not a direct UNUserNotificationCenter
implementation. The pinned
[notification-v2.4.0 desktop source](https://raw.githubusercontent.com/tauri-apps/plugins-workspace/notification-v2.4.0/plugins/notification/src/desktop.rs)
returns `Granted` from both permission methods, selects Terminal identity in
development on Mac, and dispatches `notify_rust::Notification::show()` in a task
whose result is discarded. Therefore successful `builder().show()` and a
plugin permission query cannot prove a native banner was authorized or shown.
FocusBridge's warning handler cannot recover that discarded asynchronous error.

Verify the release bundle's identity and visible behavior with notifications
allowed, denied, and suppressed by Focus. Store receipt and ACK must not depend
on a visible banner. A future native adapter or plugin change must retain the
vault checks immediately before emitting content. No attachment, quick reply,
or click-to-unlock parity is established by the current title/body builder.

## 6. Bluetooth ANCS on Mac is a different question

### Scope and official evidence

| Evidence | What it establishes | What it does not establish |
|---|---|---|
| [Apple ANCS introduction](https://developer.apple.com/library/archive/documentation/CoreBluetooth/Reference/AppleNotificationCenterServiceSpecification/Introduction/Introduction.html) | iOS is the notification provider; an accessory is the consumer, using BLE. | That any host implementing ordinary BLE can consume the system service. |
| [Apple engineer, October 2015 and March 2016](https://developer.apple.com/forums/thread/24336) | Explicit historical statement that OS X 10.11 and iOS 9 could no longer consume ANCS through CoreBluetooth; iOS continued providing ANCS to accessories. | A measured result on today's OS releases, or a ban on independent hardware consumers and Windows stacks. |
| [Core Bluetooth API overview](https://developer.apple.com/documentation/corebluetooth) | General central/peripheral communication primitives exist. | Access to every service UUID, authorized ANCS subscriptions, or a working FocusBridge adapter. |
| [CBConnectPeripheralOptionRequiresANCS](https://developer.apple.com/documentation/corebluetooth/cbconnectperipheraloptionrequiresancs) | An ANCS-related connection option exists in Apple's API documentation. | A macOS consumer capability or reversal of the historical restriction. Inspect target availability and role semantics before attempting to use it. |
| [Microsoft Phone Link setup](https://support.microsoft.com/en-us/windows/apps/phonelink/phone-link-requirements-and-setup) | Microsoft documents nearby iPhone Bluetooth pairing and Share System Notifications for its Windows product, without describing it as an EU-only accessory-framework feature. | The exact internal API, unrestricted third-party Windows ANCS access, Mac support, or remote internet delivery. |

**Conclusion:** the blanket claim that iPhone notification forwarding is
impossible outside the EU is not defensible. That does not make a stock macOS
CoreBluetooth ANCS client a supported route. Treat the latter as a separate,
high-risk feasibility experiment with explicit historical negative evidence.
No current official reversal was established in this investigation.

### Windows legacy-document discrepancy

The reported historical exclusion needs source-version care. The retrieved
[Microsoft device-capability page](https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/how-to-specify-device-capabilities-for-bluetooth)
uses the older `bluetooth.genericAttributeProfile` schema and directs Windows 10
readers elsewhere. Its fetched GATT exclusion list on this research pass contains
Human Interface Device (0x1812), **not ANCS**. This does not prove that an earlier
revision never excluded ANCS. Preserve the older claim as requiring an archived
revision, exact quotation, and applicable OS/API context rather than upgrading
it to a current platform rule.

The [modern Microsoft GATT client guide](https://learn.microsoft.com/en-us/windows/apps/develop/devices-sensors/gatt-client)
documents `DeviceCapability Name="bluetooth"`, service discovery, reads/writes,
and characteristic notifications. Generic primitives are not an ANCS service
access guarantee. Nor does the schema transition alone prove removal of an
exclusion. Phone Link establishes product behavior, not ordinary-app API parity.
Windows API/package tests and hardware candidates belong in doc 09; this report
does not certify either.

### If a Mac ANCS experiment is approved later

First prove ordinary custom-service GATT communication, then independently test
ANCS service discovery and authorized subscriptions using a signed, bundled app
and public APIs. Record Mac model, macOS build, SDK, iPhone/iOS build, Bluetooth
hardware, app identity, authorization decisions, bond state, and all operation
errors. A missing scan result alone is not a conclusive ANCS test.

The [ANCS specification](https://developer.apple.com/library/archive/documentation/CoreBluetooth/Reference/AppleNotificationCenterServiceSpecification/Specification/Specification.html)
defines service `7905F431-B5CE-4E99-A40F-4B1E122D00D0`, authorized Notification
Source, Control Point, and Data Source characteristics. Availability can change;
clients must account for Service Changed and fragmented attribute responses.
Actions are predetermined, not arbitrary text reply. Identifiers are scoped to
the ANCS session; it is not a complete cross-session synchronization service.

A useful experiment must demonstrate a real third-party notification, its
attributes, modification/removal, permission denial, disconnect/reconnect, and
content handling without guessing missing data. Repeat on more than one named
OS combination. Do not use private APIs, Notification Center database scraping,
or disabled platform protections to turn an unsupported result into a product
pass. A relay does not restore a missing Bluetooth hop. An external BLE consumer
would be an additional trusted endpoint and product design, not proof that the
Mac's own CoreBluetooth stack supports ANCS.

### Mac as source remains separate

Displaying received phone notifications is not reading notifications belonging
to other Mac applications. Apple's
[getDeliveredNotifications](https://developer.apple.com/documentation/usernotifications/unusernotificationcenter/getdeliverednotifications(completionhandler:))
returns the caller's delivered notifications. No supported universal Mac-source
API was established here. Doc 04's database path, schema, Full Disk Access
behavior, and competitor absolutes were not verified by this investigation and
must not become release promises. No scraping implementation is proposed.

## 7. Build, signing, and installation verification plan

All commands here are **future Mac verification commands**, not commands run or
results obtained during this Windows documentation task.

### Build gates

Use a Mac with the required Xcode/Command Line Tools, Rust, Node, and pnpm
available; record their versions. Check vendored SQLCipher/OpenSSL build tools
explicitly. [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

From the repository root on a Mac:

```sh
rustc -Vv
cargo -V
xcodebuild -version
xcrun --show-sdk-version
node --version
pnpm --version
perl -v
rustup target add aarch64-apple-darwin
cd desktop
pnpm install --frozen-lockfile
pnpm exec tsc --noEmit
pnpm test
pnpm build
cargo check --locked --workspace
cargo test --locked -p focusbridge-core
cargo test --locked -p focusbridge-desktop --tests
pnpm tauri build --target aarch64-apple-darwin --bundles app,dmg
```

Inspect logs for which Rust tests actually execute. The app manifest's
`[lib] test = false` matters; an aggregate cargo test success does not prove
tests nested inside `lib.rs` ran. Also run shared-engine tests explicitly from
their own manifest after returning to the repository root:

```sh
cargo test --locked --manifest-path shared/secure-channel/Cargo.toml
```

If Cargo reports a workspace or lockfile problem, record and resolve it in a
separately authorized build change; do not silently regenerate dependency locks
while calling the old snapshot verified. JNI wrapper validation is valuable
cross-platform regression coverage, but not required to link the Mac Tauri app.

For Intel/universal support, install both `aarch64-apple-darwin` and
`x86_64-apple-darwin`, then use
`pnpm tauri build --target universal-apple-darwin --bundles app,dmg` from
`desktop/`. Tauri documents that special target and both required Rust targets.
[Tauri CLI](https://v2.tauri.app/reference/cli/)

Inspect architecture slices with `file` / `lipo -archs`, native dependencies with
`otool -L`, and the generated deployment target. A universal main executable
does not establish that every bundled native component is usable on both CPUs.
Test Intel on Intel if advertised. Arm64-only is a proposed scope reduction,
not a confirmed user decision or a guarantee that native dependencies link.

### Bundle and release gates

Tauri merges `src-tauri/Info.plist` additions and supports explicit Mac
entitlements and minimum-system-version configuration. These files/settings
must be chosen and checked in a future implementation, not presumed from the
presence of `dmg` in the bundle list. Match the deployment target to every API
used; SMAppService requires macOS 13 or later unless availability-gated.
[Tauri app bundle](https://v2.tauri.app/distribute/macos-application-bundle/),
[Apple SMAppService](https://developer.apple.com/documentation/servicemanagement/smappservice)

For direct distribution, configure Developer ID Application signing and verify
the generated signature, hardened runtime, identifiers, and narrowly scoped
entitlements. Do not conflate hardened runtime with App Sandbox. Keep development
identity experiments distinct from the release identity used by Keychain.
[Tauri Mac signing](https://v2.tauri.app/distribute/sign/macos/)

Submit the intended release container with `notarytool`, retain the status and
log, then staple and validate the relevant distributable/app tickets. A returned
submission ID is not acceptance. Do not mutate signed bundle contents after
signing. Apple accepts ZIP, DMG, and signed flat installer containers; a bare
app directory is not a submission container.
[Apple notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow)

Example checks once a release bundle exists:

```sh
APP="/Applications/FocusBridge.app"
codesign --verify --deep --strict --verbose=2 "$APP"
codesign -dv --verbose=4 "$APP"
codesign -d --entitlements :- "$APP"
spctl --assess --type execute --verbose=4 "$APP"
xcrun stapler validate "$APP"
```

These checks are necessary evidence, not proof of correct installation or runtime
behavior. Record artifact SHA-256, bundle version, executable architecture, Team
ID, and signing identity; compare the installed app with the intended payload.
Test the browser-downloaded, quarantined DMG on a clean account/Mac with no
developer tools or previous approvals. Also verify launch with the ticket
available offline. Do not use quarantine removal or Gatekeeper bypass as the
release acceptance path. A normal first-open confirmation is not the same as an
unidentified-developer/damaged-app rejection; avoid promising zero OS dialogs.

CI should separate unprivileged checks from explicitly authorized release
signing, avoid exposing certificates to untrusted PRs, and upload native build,
test, and notarization reports with secrets redacted. No release workflow or
certificate setup was performed here.

## 8. Existing tests versus evidence still required

| Existing suite / code | What it can establish when actually run | Remaining gap |
|---|---|---|
| `core/src/qr.rs:367` and subsequent tests | Codec round trips, length budget, refusal of truncated/padded input, fallback, Android-compatible fields. | Real Mac display and Android scan; address candidates must be reachable. |
| `core/src/protocol.rs`, `handler.rs`, `priority.rs`, `study_mode.rs` | Shared serialization, handler decisions, and rule behavior. | Native runtime and complete Android/Mac parity. |
| `shared/secure-channel/tests/sessions.rs`, `public_vector.rs`, `src/limits_tests.rs` | Session authentication, enrollment, replay/tamper rejection, fragmentation/limits, independent vector matching. | Real Mac transport integration, entropy/toolchain build, persisted identity, and reconnect. |
| `src-tauri/tests/encrypted_database.rs` | SQLCipher secrecy, migrations, crash helpers, WAL, wrong-key refusal, connection guards. Most fixtures inject a test key. | Production signed-app key acquisition, APFS publication/recovery, and real Keychain permission behavior. |
| `security/database_key.rs:121` | A macOS-only disposable Keychain roundtrip and deleted-key refusal test exists. It can be included through the integration tests' path-imported DB module. | Prove its name appears in the Mac test listing and it executes; it bypasses parts of production path/default-keychain acquisition. |
| `src-tauri/tests/app_inventory.rs` | Inventory validation, transactional rollback, metadata/preferences, migration behavior with explicitly keyed storage. | End-to-end Android inventory and icons in the actual WKWebView. |
| `src-tauri/tests/connection_state.rs` | Ownership, manual disconnect, heartbeat/socket deadlines and pending-frame handling; persistence is stubbed. | Real TLS/relay supervisors, DB persistence, network change, sleep/wake, and user-driven reconnect behavior. |
| Unit tests inside application modules | Useful test definitions exist for TLS initialization, vault listing, pairing effects, relay enrollment, and attach decisions. | `[lib] test = false` means definitions alone do not establish execution. Inventory the actual test targets and add reachable coverage later. |
| Frontend Vitest suites | Renderer/state utilities and mocked component behavior. | WKWebView, native IPC, notification authorization, menu behavior, and Keychain cannot be inferred from jsdom success. |

The current [Tauri WebDriver guide](https://v2.tauri.app/develop/tests/webdriver/)
now documents a WebdriverIO embedded-server route for macOS. Direct
`tauri-driver` remains Windows/Linux-only; a paid alternative is also described.
Do not repeat the obsolete blanket claim that no Mac automation route exists.
None is configured in the inspected repository. Any added instrumentation must
be kept out of production release builds, and native permission/hardware tests
still need their own evidence.

### Release acceptance record to produce

- [ ] A named macOS/CPU/toolchain build links, launches, and loads the full UI; unsupported architectures/OSes are stated explicitly.
- [ ] Unmodified Android pairs on ordinary Wi-Fi and an offline LAN, receives a stored/ACKed notification, inventory, and rules updates; bad pins/authentication fail closed.
- [ ] The same Android on mobile data delivers through the relay and pinned loopback; local-only and relay-disabled behavior remain truthful.
- [ ] Manual disconnect stays disconnected; QR preview alone does not resume; explicit scan/reconnect and automatic-reconnect preferences behave as designed on both transports.
- [ ] VPN, hotspot, Ethernet, changed address, occupied port, captive/failed internet, and sleep/wake outcomes are diagnosable without silently advertising loopback to the phone.
- [ ] Locked startup/relock prevent inbox disclosure and new message banners while background storage continues; preexisting native notification previews have a documented policy.
- [ ] SQLCipher and Keychain failure/recovery cases pass without resetting keys or losing migration artifacts; TLS private-key files have appropriate access controls.
- [ ] Native notification authorization/suppression, menu/Dock behavior, close versus Quit, login opt-in/revocation, and logout/login are tested in the signed app.
- [ ] Generated metadata, minimum OS, architectures, signatures, notarization, staples, artifact identity, and clean quarantined installation are recorded.
- [ ] Existing Windows/Android behavior remains covered; source reuse is not substituted for regression evidence.

All boxes are deliberately unchecked. No timeline, cost, App Store admission,
Mac ANCS access, universal binary success, permission prompt sequence, or
production parity is guaranteed by this report. The next useful milestone is a
linked, signed Mac receiver spike with recorded failures and successes, not a
claim that all Apple-platform features now work.
