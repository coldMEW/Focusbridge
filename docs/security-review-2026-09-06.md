# Security review, 2026-09-06

Covers the cross-network relay work and a sweep of the surfaces it touches.
This is a scoped review, not a certification that every line in the repository
is correct, and it is not a substitute for an independent audit before a public
release.

## Fixed in this pass

### A rejected database key rewrote the database

Opening the encrypted database with a wrong key still checkpointed the
write-ahead log into the main file as the failed connection was dropped. The
frames are ciphertext, so a wrong key did not stop it: a rejected unlock attempt
silently rewrote 8,164 bytes of a database whose owner may still have been
trying to recover it. The key probe now suppresses checkpoint-on-close until the
key has been proven, and a test asserts the file is byte-identical afterwards.

### Any app or web page could silently re-pair the phone

`focusbridge://pair` was consumed with no confirmation. The intent filter is
`BROWSABLE`, so a web page could send one, and the version 2 payload carries
relay credentials — meaning a single link could have redirected a phone's
notifications to a stranger's PC, from any network, with nothing shown to the
user. Pairing links now show the address, whether the PC will be reachable off
the local network, and the certificate code the desktop displays, and wait for
the user to accept. A source guardrail test asserts the payload is only consumed
from a confirmation button.

### Credentials could be posted to an arbitrary host

Three Tauri commands outlived the UI panel that called them:
`auth_relay_otp_start`, `auth_relay_otp_verify` and `auth_google_sign_in`.
Removing a panel does not unregister a command. They took a relay URL from the
caller, prepended `https` only when no scheme was present — so `http://` passed
through untouched — and posted an account password, an emailed one-time code, or
a Google ID token to whatever address they were given. They pointed at a relay
that is not deployed, so nothing worked through them; what remained was a way to
send credentials somewhere else. Removed, along with the helpers and the PKCE
module that had no other user.

### The local listener had no connection ceiling

Bound to `0.0.0.0`, spawning a task per connection with no limit. Each costs a
TLS handshake, buffers and a task for up to the authentication deadline, so a
few hundred idle sockets made the machine do unbounded work for an
unauthenticated peer. Capped at 24 in flight, excess dropped rather than queued.
Measured against the running app: 200 idle sockets produced 179 refusals, memory
held at 45 MB, and a legitimate TLS handshake succeeded again once the deadline
recycled the slots.

### Cleartext traffic was still permitted

`usesCleartextTraffic` was true from an earlier LAN-only design, although every
transport is now TLS: the local listener is WSS with a QR-pinned certificate and
the relay is WSS to a public HTTPS origin. Set to false so a regression cannot
silently downgrade one.

### Request logging in release builds

The OkHttp logging interceptor ran at BASIC in every build. Request lines carry
the desktop's local address and the relay account and pair identifiers, and
logcat is readable by more of the system than this app's own storage. Disabled
outside debug builds.

## Checked and found sound

- **SQL.** Every statement is parameterised. The one query built with `format!`
  interpolates a column name that comes from a closed `match` over three literals
  and rejects anything else.
- **Panics from network input.** No `unwrap` or `expect` on parsed or received
  data in the server, relay client, protocol or envelope code; the only ones are
  in tests.
- **Content in logs.** Neither platform logs notification bodies, senders,
  titles, pairing keys, capabilities or key material. Relay errors carry a reason
  string, never a credential.
- **Android exported surface.** A content provider is not declared. The only
  exported components are the launcher activity, whose deep link is now behind a
  confirmation, and a boot receiver guarded by
  `android.permission.RECEIVE_BOOT_COMPLETED`.
- **Backups.** `allowBackup` is false, so the pairing row holding the relay
  capability and the pre-shared key is not exported by Android backup.
- **Key separation.** The phone identity and the database passphrase use
  distinct keystore aliases, records and additional authenticated data, so
  neither wrapped secret can be substituted for the other. The desktop identity,
  per-pair secret and pinned phone key live in the SQLCipher settings table.
- **Relay authority.** The relay stores capability hashes only, cannot
  manufacture an acknowledgement, cannot report a phone as authenticated, and
  keeps no application data. Revoking a pair does not touch device key material.

## Evidence that traffic is unreadable in transit

- The shared engine's tests assert that a sealed frame does not contain its
  plaintext, and that **each chunk** of a large record hides its contents
  independently, using a payload shaped like a real app inventory. That matters
  because every chunk crosses the relay on its own.
- Tampering, reflection, replay, truncation, reordering, a wrong peer identity,
  a wrong pre-shared key and a mismatched pair context all fail closed, and any
  failure permanently retires the session rather than resuming it.
- The relay refuses text frames outright and forwards only opaque binary, so it
  has no path to application content even if it wanted one.
- On the phone, an on-device test asserts the inner envelope does not contain
  the message it carries. It runs on a device deliberately: `android.util.Base64`
  returns defaults under the JVM runner, where a broken implementation would
  pass.

## Known and accepted

- **The pairing payload is a secret while it is displayed.** It carries the
  pre-shared key and the phone's relay capability, and the manual payload is
  selectable text. It expires in five minutes, and enrollment additionally
  requires the pairing screen to be open. Do not paste it anywhere.
- **A sustained flood can delay pairing.** Twenty-four slots held by an attacker
  on the same network will refuse a legitimate connection until the
  authentication deadline recycles them. The app recovers; it is not prevented
  from working, only slowed.
- **A relay session is only as private as the endpoints.** Nothing here defends
  against malware already running on a paired machine.
- **`workers.dev` is documented by Cloudflare for personal use.** A
  business-critical deployment wants a custom domain and a route.

## Not yet done

- No independent audit, and no third-party review of the cryptographic
  integration.
- Release signing still uses a debug key; the release APK is not distributable.
- Abuse controls on the desktop's own account endpoints have not been
  re-examined since the legacy relay auth was removed.
- No overnight endurance run, and no broader Android or OEM matrix.
