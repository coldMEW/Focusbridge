# FocusBridge

**Your phone's notifications, on your PC. From any network. Without your phone in your hand.**

FocusBridge sends the notifications that matter from your Android phone to your
Windows desktop, filters out the ones that don't, and keeps working whether the
two devices share a Wi-Fi network or are on opposite sides of the internet.

## The problem

You put your phone face-down to focus, and then you pick it up anyway — because
you might be missing something. Usually you aren't. But checking costs more than
the glance: you unlock the phone, and twenty minutes later you are somewhere else
entirely.

The phone is not the problem. Not knowing is.

FocusBridge moves the knowing to the screen you are already looking at, and
applies a filter on the way, so a two-factor code reaches you instantly and a
sale notification does not reach you at all.

## What it does

**Notifications on your desktop.** Messages, codes, calls, delivery updates —
they appear on your PC as they arrive on your phone, with the app icon, sender
and time.

**Rules that actually silence things.** Mute whole apps. Promote the ones that
matter. Flag priority keywords and favourite contacts so they always surface.
Block keywords so promotional noise never leaves the phone. The rules are set on
the desktop and enforced *on the phone*, so filtered notifications are never
transmitted at all.

**Study Mode.** One switch that suppresses low-priority noise while leaving
urgent alerts and security codes visible.

**Masked Peek.** Notification bodies stay hidden behind a chip until you hover
or click. Useful when your screen is shared or someone is standing behind you.

**Triage, not a second inbox.** Pin what needs an answer, dismiss what doesn't,
clear by age, and keep a searchable log of what came through.

**Two ways to connect, automatically.** On the same Wi-Fi, your devices talk
directly to each other and nothing leaves your network. Off it — phone on mobile
data, laptop on café Wi-Fi, either device behind a router that blocks incoming
connections — they meet at a relay instead. FocusBridge prefers the local path
whenever it exists and falls back without you doing anything.

**Reconnect from the PC, from anywhere.** Pick a phone under previous
connections and it reconnects, even when the two are on different networks and
neither can dial the other. A disconnected phone stays reachable for exactly
this reason: it holds no session and sends nothing, it is simply somewhere the
PC can reach it.

**You decide what reconnects.** Both ends have a switch. On, a device you have
paired before reconnects without asking. Off, nothing connects silently — the PC
waits until you choose a phone, and the phone notifies you before letting a PC
in. It is one decision on each side, and it is the only thing that decides.

## Privacy

Notification content is readable by exactly two machines: your phone and your
paired PC.

- **Nothing is stored in a cloud.** There is no server holding your messages.
- **The relay cannot read what it carries.** When your devices are on different
  networks, traffic passes through a relay — sealed end to end, with keys that
  never leave your devices. The relay routes bytes it has no way to decrypt, and
  keeps nothing.
- **On disk it is encrypted too.** Notification history is stored encrypted on
  both the phone and the PC, with the key held by the operating system's own
  keystore rather than a password you type.
- **Pairing is deliberate.** A PC gets access only when you scan its code and
  confirm, and only after you have compared the security code it shows. A
  pairing link cannot connect anything on its own, however it arrives. You can
  revoke a device at any time, from either end.
- **Local sync needs no account.** Same-network use works with no sign-in and no
  internet connection at all. An account is only needed to turn on the
  cross-network relay.

## How it is built

| Part | Technology |
| --- | --- |
| Phone app | Kotlin, Jetpack Compose, Room, Hilt, CameraX |
| Desktop app | Tauri, Rust, React, TypeScript |
| Local link | WebSockets over TLS, with the desktop's certificate pinned from the pairing code |
| Cross-network link | Cloudflare Workers with SQLite Durable Objects, on the free tier |
| End-to-end encryption | Noise protocol (`Noise_XXpsk3_25519_ChaChaPoly_SHA256`), one Rust implementation shared by both platforms |
| Storage | SQLCipher, keyed through Windows DPAPI and the Android Keystore |
| Accounts | Firebase Authentication, used only to authorize the relay |

The encryption engine is deliberately written once, in Rust, and loaded by the
Android app through JNI. Two implementations of the same protocol are two
implementations that can disagree, and a disagreement in a handshake is a
security bug.

## What it is not

FocusBridge mirrors notifications. It is not a full phone-on-your-desktop
product: it does not send SMS, place calls, mirror your screen, or transfer
files. Those are deliberately out of scope rather than half-built, and the
comparison against Microsoft Phone Link is written down honestly in
[`docs/phone-link-gap-analysis.md`](docs/phone-link-gap-analysis.md).

It also cannot reach a phone that is switched off, out of signal, or has been
force-stopped by the system, and no notification app can.

## Security

Reviewed in depth, with the findings and the remaining gaps written down in
[`docs/security-review-2026-09-06.md`](docs/security-review-2026-09-06.md)
rather than summarised away. Among the things it covers: a rejected database key
can no longer alter the database, a pairing link can no longer connect a phone
without being accepted, the local listener bounds how many connections an
unauthenticated peer can hold, and nothing logs notification content.

What has not happened is stated just as plainly: there has been no independent
audit, release signing still uses a debug key, and no overnight endurance run.

## Status

Working and in daily use by its author. Cross-network sync is verified on real
hardware: a Pixel 7 on cellular only, with no Wi-Fi at all, reaching a Windows
PC on home Wi-Fi through the relay.

Release signing and a broader device matrix are still outstanding before a
public release, and the remaining gates are tracked in [`docs/`](docs/).

## Documentation

- [Cross-network architecture](docs/cross-network-architecture.md) — how the two
  transports work and what each component is trusted with
- [Phone Link comparison](docs/phone-link-gap-analysis.md) — feature-by-feature,
  including what is missing
- [Security model](docs/security-model.md) and [privacy policy](docs/privacy-policy.md)
- [Architecture](docs/architecture.md)
- [Cross-network acceptance run](docs/cross-network-acceptance.md) — how to
  reproduce the different-network test, and what has and has not been run

## Licence

See [LICENSE](LICENSE).
