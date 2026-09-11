# Porting the desktop app to macOS

The unblocked half of the expansion. Nothing here is impossible; it is a list of
substitutions and one genuine hazard (the local-network permission).

Target: the same Tauri 2 app, same React front end, same Rust back end, same
protocol, running natively on Apple silicon and Intel Macs, paired with the
existing Android app with no protocol change.

---

## 1. Component-by-component

| # | Windows today | macOS equivalent | Difficulty | Risk |
|---|---|---|---|---|
| 1 | Tauri 2 + WebView2 | Tauri 2 + WKWebView | Low | Low |
| 2 | TLS WebSocket server on `0.0.0.0:9173` (tokio + tokio-rustls + tokio-tungstenite) | Identical Rust | Low | **Local-network permission — see §3** |
| 3 | Self-signed cert generated at first run, pinned by DER SHA-256 | Identical Rust | Low | Low |
| 4 | Relay client dialling Cloudflare on 443 | Identical Rust | Low | Low |
| 5 | Loopback bridge to own listener | Identical Rust | Low | Medium — loopback is not "local network", but verify TCC does not intercept |
| 6 | SQLCipher via `rusqlite` `bundled-sqlcipher-vendored-openssl` | Same crate, but see §5 | Medium | Medium |
| 7 | **Windows DPAPI** wrapping the DB key | **Keychain**, optionally Secure-Enclave-wrapped | Medium | Medium — see §4 |
| 8 | Windows toast notifications | `UNUserNotificationCenter` | Medium | Medium — requires a signed, bundled app |
| 9 | System tray + menu, close-to-tray | `NSStatusItem` (Tauri tray), `LSUIElement` | Low | Low |
| 10 | `run_windows_first_run_setup` (firewall, autostart) | `SMAppService` for login items; no firewall step needed | Low | Low |
| 11 | Interface enumeration, routed adapter first | Same idea, different interface names to exclude | Low | Low |
| 12 | MSI installer + hash-verified install script | `.app` in a `.dmg`, notarised and stapled | Medium | Low |
| 13 | Firebase Auth (JS SDK in the webview) | Unchanged — it is web code | Low | Low |
| 14 | CSP + Tauri capabilities | Unchanged | Low | Low |
| 15 | GitHub Actions `windows-latest` | Add `macos-latest` | Low | Low |

Items 1–5, 11, 13, 14 are effectively free: the code is portable Rust and web.
The work is concentrated in 6, 7, 8, 12 and the permission story.

---

## 2. Tauri on macOS

Tauri 2 reached stable in October 2024 and its **desktop** support is production
grade **[REPORTED]**. Its mobile support is younger — relevant to
[`03-ios-app-engineering.md`](03-ios-app-engineering.md), not here.

Build a universal binary:

```
pnpm tauri build --target universal-apple-darwin --bundles app,dmg
```

which `lipo`-joins the arm64 and x86_64 slices into one `.app` **[REPORTED]**.
Add both Rust targets first:

```
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

Caveat worth knowing in advance: Tauri produces a universal binary for the **Rust**
binary only. Any sidecar or external binary must be built universal yourself with
`lipo` **[REPORTED]**. FocusBridge has no sidecar today; keep it that way.

### WKWebView differences to expect

The front end is React + Vite with a strict CSP. Differences that have bitten
other projects and should be checked early, not assumed:

- Custom protocol / asset loading differs from WebView2; Tauri abstracts it but
  CSP strings sometimes need a macOS-specific entry.
- `localStorage` and IndexedDB persistence inside a WKWebView is tied to the app's
  container; verify the settings store survives a restart.
- Devtools must be explicitly enabled in debug builds.

None of these is a blocker; all are things to verify in the first hour rather than
discover in week three. **[UNVERIFIED — no macOS build has been attempted.]**

---

## 3. The real hazard: local network permission

**This is the one thing that can make the app appear broken on macOS in a way it
never does on Windows.**

macOS 15 Sequoia introduced a local-network privacy prompt for Mac apps, matching
the one iOS has had since iOS 14. An app that connects to, or listens on, the
local network triggers a user consent dialog, and is blocked until granted
**[REPORTED]**.

What the community reports, and what to plan around **[REPORTED]**:

- Apps sometimes **do not trigger the prompt at all**, leaving the user with no
  way to grant it.
- A first `sendto()` can fail immediately with `EHOSTUNREACH` before the user has
  had any chance to consent.
- Connections to local-network machines have been reported to fail entirely **when
  the app is not in `/Applications`** — which is exactly where a developer's build
  is not.
- Permission has been reported as ignored after a reboot despite being granted.

Consequences for FocusBridge's design:

1. **Ship `NSLocalNetworkUsageDescription` in `Info.plist`** with a sentence that
   explains the pairing, not a generic string. Reported as not strictly enforced
   yet, but omit it and the dialog is unexplained.
2. **The app must be run from `/Applications`.** The DMG should make that the
   obvious action, and the app should detect that it is running from elsewhere
   and say so, because the symptom otherwise is "pairing does not work" with no
   error — precisely the class of bug that cost this project days on Windows (see
   [`../BLUEPRINT.md`](../BLUEPRINT.md) §6.29).
3. **Diagnostics must be able to say "local network permission was refused."**
   The existing `get_connection_diagnostics` already reports the last disconnect
   reason; add a macOS-specific probe. A silent failure here would be the same
   shape of bug as "it will not connect" versus "it was never configured", which
   commit `4501bf7` fixed once already.
4. **The relay path must keep working when local network is denied.** It should,
   because the relay is an outbound 443 connection to a public host — not local
   network. That makes the relay the *fallback for a permission failure*, not just
   for a routing failure. Worth an explicit test.
5. **The loopback bridge** (`relay_client.rs` dialling 127.0.0.1:9173) should not
   be treated as local network by TCC. **[UNVERIFIED — must be tested; if it is
   intercepted, the whole relay path breaks on macOS and the bridge would need to
   become an in-process channel instead of a loopback socket.]** This is risk
   R-10 and deserves an early spike because it touches the architecture.

### Would Bonjour be better than IP addresses?

For LAN discovery, `NWBrowser`/mDNS would be more idiomatic than baking interface
addresses into the QR. It also triggers the same permission. Recommendation: **do
not change the pairing model for macOS.** The QR already carries candidates and
the routed-adapter-first ordering fix (`fab5b75`) applies equally. Revisit Bonjour
only if the permission story forces it.

---

## 4. Replacing DPAPI

Windows DPAPI wraps the SQLCipher key so it is bound to the user and machine and
never derived from a typed password — which is what lets background sync keep
running while the UI is locked. macOS needs the same property.

**Recommendation: Keychain (`kSecClassGenericPassword`), with the item marked
non-synchronising and accessible after first unlock, and an explicit access
control that does not require user presence.**

Rationale, and the alternatives rejected:

| Option | Verdict |
|---|---|
| **Keychain generic password** | **Chosen.** Directly analogous to DPAPI: OS-managed, bound to the user's login keychain, no typed password. Reachable from Rust via the `security-framework` crate. |
| Secure Enclave key wrapping the DB key | Rejected for v1. The Enclave holds P-256 keys only, so the symmetric key must be wrapped by ECIES through `SecKeyCreateEncryptedData` — more moving parts, and the Enclave key is destroyed on certain system events, which would mean an unrecoverable database. Revisit as a hardening step, never as the only copy. |
| Touch ID gating via `LAContext` | Rejected as the *storage* mechanism. It belongs to the app-lock feature, not the database key; gating the DB key on user presence would break background sync, which is the exact thing DPAPI was chosen to avoid. |
| Derive from a user password | Rejected — this is what the project already decided against on both platforms. |

Hazards to design for **[UNVERIFIED — all need testing on a real Mac]**:

- **Re-signing the app with a different identity changes its ACL position**, and
  macOS can start prompting "FocusBridge wants to use your confidential
  information" on every launch. This bites when moving from a development
  signature to a Developer ID one. Plan the key's creation to happen *after*
  signing identity is settled, and provide a documented recovery path
  (re-derive/recreate the database) rather than a dead app.
- The keychain item must be created with the app as a trusted application so the
  prompt appears once, not per access.
- Migration: a Mac install is always a fresh database in v1. Do not attempt to
  import a Windows database — the key is machine-bound by design and cannot move.
  Say so in the UI rather than failing mysteriously.

---

## 5. SQLCipher on macOS

Today: `rusqlite` with `bundled-sqlcipher-vendored-openssl`. On Windows this
needed a native Perl and cost this project real time
([`../BLUEPRINT.md`](../BLUEPRINT.md) §6.26).

On macOS, Perl is present in the base system, so the vendored-OpenSSL build is
expected to work **[UNVERIFIED]**. The genuine question is the **universal binary**:
a vendored OpenSSL must be built for both architectures and `lipo`-joined. Two
approaches:

1. **Build each architecture separately and `lipo` the final binaries.** This is
   what `--target universal-apple-darwin` does, and it means OpenSSL is compiled
   twice, natively, per architecture. Slow but correct.
2. **Ship arm64-only for v1.** Apple silicon has been the only Mac sold since
   2023; an Intel build can follow. This removes the universal-OpenSSL question
   entirely and halves CI time.

**Recommendation: arm64-only for the first release**, with universal as an explicit
later gate. Say so in the release notes rather than silently excluding Intel users.

Rejected alternatives: linking SQLCipher from Homebrew (creates a machine-specific
dependency that will not be present on a user's Mac); dropping SQLCipher for
Apple Data Protection file classes (macOS file protection is weaker than iOS's,
and it would fork the storage design across platforms for no gain).

---

## 6. rustls and TLS

The LAN path pins a self-signed certificate by DER SHA-256; it never consults a
system trust store. That makes the platform verifier irrelevant for the path that
matters, and removes the usual macOS trust-store friction.

- Keep the existing provider choice and the startup `install_crypto_provider()`
  call. The lesson from `edad4c6` — rustls 0.23 panics at runtime on a worker
  thread if no provider was chosen, and every test passes anyway — applies
  identically on macOS. The existing
  `tls_provider_tests::a_tls_provider_is_chosen_before_any_connection_is_made`
  test protects the Mac build for free.
- The relay path dials a public Cloudflare host and *does* need normal CA
  validation; the platform's roots via the standard webpki roots are fine.
- **[UNVERIFIED]** Whether the chosen provider builds cleanly for both Apple
  architectures is a first-hour check, not an assumption.

---

## 7. Native notifications, tray and login

### Notifications

`UNUserNotificationCenter` is the modern API and works for Mac apps, but with
conditions that differ sharply from Windows toasts:

- The app must be **signed and properly bundled**; an ad-hoc build will behave
  differently or not at all. **[UNVERIFIED — verify early, because it interacts
  with the signing timeline.]**
- The user must grant notification authorisation; handle denial explicitly. The
  inbox must still fill (this is exactly the `35683ae` lesson: storage and display
  are separate concerns, and a display failure must not lose data).
- Focus / Do Not Disturb will suppress alerts. The desktop should not treat
  suppression as a delivery failure.
- Attachments: the current UI shows the app icon with each notification. A macOS
  notification can carry an image attachment, which covers it.

Tauri's notification plugin may be sufficient; if not, a small `objc2` bridge is
the fallback. Decide after the first spike.

**Design rule to carry over:** the vault gate. `35683ae` and `42ca81f` established
that the backend owns the lock and refuses to emit or list while locked. That must
hold on macOS from the first commit, not be retrofitted — it is rows in
[`../behaviour-checklist.md`](../behaviour-checklist.md) already.

### Tray

Tauri's tray maps onto `NSStatusItem`. Two macOS-specific behaviours to get right:

- **Close-to-tray is not the Mac idiom.** On macOS, closing a window leaves the
  app running and the Dock icon present; quitting is ⌘Q. Match the platform:
  closing the window hides it, the menu bar item stays, ⌘Q quits.
- Consider `LSUIElement` (menu-bar-only, no Dock icon) as a setting rather than a
  default. Users differ strongly on this.

### Launch at login

`SMAppService` (macOS 13+) replaces the old login-item APIs. This substitutes for
part of `run_windows_first_run_setup`. There is no firewall step to replicate —
macOS's application firewall prompts on first listen and is a separate consent
from the local-network one. Expect **two** prompts on first run and explain them
in the UI before they appear.

---

## 8. Distribution

See [`06-distribution-and-cost.md`](06-distribution-and-cost.md) for the money.
Mechanically:

1. `codesign` with a **Developer ID Application** certificate, hardened runtime
   on, and the entitlements file.
2. `xcrun notarytool submit --wait`, then `xcrun stapler staple`.
3. Bundle into a `.dmg` whose window makes dragging to `/Applications` the
   obvious action — which §3 makes functionally necessary, not cosmetic.
4. Gatekeeper then admits the app with no right-click-Open dance.

**Mac App Store is not recommended for v1.** The App Sandbox would require
`com.apple.security.network.server` and `com.apple.security.network.client`, and
the loopback-bridge architecture plus a listening socket on a fixed port is the
kind of thing that invites review questions for no user benefit. Direct notarised
distribution matches how the Windows build already ships.

**Install verification.** The Windows script compares the installed binary against
the payload extracted from the installer that was run (`42ca81f`). The macOS
analogue is simpler and stronger: `codesign --verify --deep --strict` plus
`spctl --assess`. Write the equivalent of `install-desktop.ps1` as a shell script
that does that, and keep the same principle — "it did not work" must be
distinguishable from "it was never installed".

---

## 9. CI

Add a `macos-latest` leg. `desktop-ci.yml` already has a matrix including
`macos-latest` for `cargo check`; extend it to a real build.

- Signing and notarisation **can** run in Actions, with the certificate as a
  base64 secret imported into a temporary keychain, plus an App Store Connect API
  key for `notarytool` **[REPORTED]**. Do not put these on `push`; put them on
  tags only.
- macOS runners are billed at a higher multiplier than Linux on private repos;
  for a public repository they are free **[UNVERIFIED — confirm current GitHub
  billing terms before relying on it]**.
- **Keep it deterministic.** The Android CI is currently flaky at ~50%
  ([`../BLUEPRINT.md`](../BLUEPRINT.md) §9) and it has cost real diagnostic time.
  Do not repeat the mistake: on the macOS legs, upload test reports on failure
  from day one, and avoid wall-clock assertions in any new test.

---

## 10. What this does not change

Worth stating, because it is the reason this port is cheap:

- The wire protocol is unchanged. A Mac and a Windows PC are the same peer to the
  Android app.
- `focusbridge_core` — QR encoding, protocol types, priority, study mode — is
  unchanged.
- The relay is unchanged. No Worker deployment is needed for macOS support.
- The Android app needs **no changes at all** to pair with a Mac. The pairing QR
  already carries everything, and the certificate pinning is platform-neutral.

That last point is the strongest argument for doing this first: it is the only
piece of the Apple expansion that ships value without touching anything that
currently works.
