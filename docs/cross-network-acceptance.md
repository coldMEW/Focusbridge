# Cross-network acceptance run

The one gate that cannot be automated here: the relay only provisions a pair for
an email-verified account, so a person has to sign in. Everything else is ready.

Record the result at the bottom. Until it is filled in, cross-network sync is
built and component-tested but **not** proven on real networks.

## Before starting

- Desktop app running from
  `desktop/target/release/focusbridge-desktop.exe`, or install
  `desktop/target/release/bundle/msi/FocusBridge_1.0.0_x64_en-US.msi`.
- Phone has the current build (`adb install -r app-debug.apk`).
- Relay is live: `curl https://focusbridge-relay.focusbridge.workers.dev/health`
  returns `{"status":"ok","protocol":1}`.

## Steps

1. **Sign in.** Desktop → account screen → sign in or create an account with a
   real email address.
2. **Verify the email.** Settings → Cross-network sync → *Resend verification
   email*, then open the link in the mailbox. The relay refuses unverified
   accounts, and this is the only thing standing between an email address and a
   route to your phone.
3. **Turn on cross-network sync.** Same panel. It should change to *Waiting for
   your phone* and show a renewal date about 30 days out.
   - "Sign in again" means the token expired — sign in and retry.
   - "Confirm your email address first" means step 2 has not taken effect yet;
     Firebase needs the link clicked before it will mint a verified token.
4. **Get a fresh QR.** Desktop → Pair. The QR now carries the relay block, so an
   older QR will not work for this.
5. **Put the phone on a different network.** Turn Wi-Fi **off** so it is on
   mobile data only. This is the case that never worked before.
6. **Pair.** Open FocusBridge on the phone, scan the QR, and confirm the dialog.
   Check the security code it shows matches Settings → diagnostics on the
   desktop.
7. **Watch it connect.** Desktop diagnostics should show connected with
   transport `relay` — not `wss`, which would mean it found a LAN route after
   all. The cross-network panel should read *Cross-network sync is on*.
8. **Send a notification.** Anything on the phone. It should appear on the
   desktop while the phone has no Wi-Fi at all.

## Then check the failure paths

These matter more than the happy path, because they are where a sync product
usually lies to the user.

| Case | Do this | Expected |
| --- | --- | --- |
| LAN preferred | Turn phone Wi-Fi back on, same network as the PC | Reconnects and diagnostics switch to `wss`; not two copies of each notification |
| Peer offline | Quit the desktop app, send a phone notification, reopen | Queued notification arrives once, not twice |
| Relay outage | Turn the PC's network off for a minute, then back on | Reconnects on its own; no permanent disconnected state |
| Doze | Leave the phone idle and screen-off for 30+ minutes, then send one | Still delivered, or delivered on the next wake |
| Full disconnect | Desktop → *Disconnect phone* | Phone shows disconnected and does not silently reconnect |
| Revocation | Settings → *Turn off cross-network sync* | Phone can no longer reach the PC off-LAN, even with the old QR |

## Result

- Date:
- Phone network / PC network:
- Connected over relay: yes / no
- Notification delivered off-LAN: yes / no
- Failure paths that behaved: 
- Anything that broke: 

## If it does not connect

Collect these before changing anything:

```
adb logcat -d | findstr /i focusbridge
```

and the desktop log (run it with `RUST_LOG=info`). The useful lines are
`relay socket established`, `relay secure session ready`, and any
`relay session ended` with its reason. A `relay.peer_unavailable` simply means
the other side has not arrived yet, which is normal until both are connected.
