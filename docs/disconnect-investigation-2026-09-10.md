# Why the connection kept dropping — 10 September 2026

The report: FocusBridge disconnects by itself while both devices are plainly
online. Sometimes after thirty seconds, sometimes two minutes, sometimes two or
three hours, sometimes instantly.

That spread is the shape of the answer. Nothing in the code fires at thirty
seconds *and* at three hours, so the interval was never a timer — it was
"however long until the first hiccup", and the hiccup was fatal when it should
have been invisible.

## What could be measured

The app produced **no logs at all**. It is a windowed process on Windows, so it
has no console for standard output to reach, and its filter came from `RUST_LOG`
with no default, which nothing sets on an installed copy. So the first step was
not a fix: it was relaunching the installed binary with its output redirected to
a file, and watching.

That is now permanent — see "Fix 3" below — so the next surprise can be read
rather than reconstructed.

The installed binary was checked first, and is **not** stale: it is dated
2026-09-09 22:50, newer than every source change in the working tree, including
the earlier heartbeat work. The heartbeat fix was already in the copy that was
misbehaving, so it was not the cause and was not the fix.

## Root cause 1 — two features were sharing one flag

"Reconnect to the last phone automatically" was doing two unrelated jobs:

1. **The first connection.** When the desktop starts, or when a saved phone
   turns up unasked, may this PC take it? This is a preference, and it belongs
   entirely to the user. Off, deliberately, on this machine: the user does not
   want the last known phone grabbed automatically.

2. **Keeping a live connection alive.** Once a phone is connected, the transport
   underneath occasionally drops — a relay socket replaced, Wi-Fi handing over,
   a lost packet. Putting it back is plumbing. The user already said yes to that
   phone; they are not being asked again every time a radio blinks.

Only (1) was ever meant to be a setting. But the rule was enforced in
`may_attach` on **every socket**, with a **single-use allowance** — so turning
off (1) also turned off (2). Picking the phone by name granted exactly one
attach. The moment anything underneath flinched, the phone came back, the
allowance was already spent, and it was refused. Permanently, because only
another manual pick could grant a new one.

That is the whole variability. The connection did not die on a timer; it died on
the first blip, and the blip could come in thirty seconds or three hours.

Captured live, on an idle machine with a stable connection:

```
deciding whether this phone may attach peer=127.0.0.1:54719
    paused=false automatic=false holds_the_code_on_screen=false
refused a phone; it was disconnected here or automatic reconnection is off
```

`paused=false` — the user had not disconnected. The phone was refused purely by
a setting that was answering a question it was never asked.

The two are now separate and named apart in `focusbridge_core::attach`:
`auto_first_connection` (the user's) and `resuming_the_users_connection` (the
backend's). The module documents why they must not be folded back together.

## Root cause 2 — a refused phone was re-dialled in a loop, kicking it each time

Once refused, the desktop did not settle. Measured from the live log:

```
18:34:51  relay socket established … refused a phone
18:35:26  relay socket established … refused a phone
```

Thirty-five seconds, repeating: thirty seconds of `await_relay_request`'s
safety-net timeout, plus five seconds of backoff. `start()` then recomputed
`automatic` as `auto_connect || pairing_code_is_live()`, and the pairing panel
renders by itself whenever nothing is connected — so the code on screen forced
another dial, into the same refusal. **32 cycles and 32 refusals** were recorded
in twenty-six minutes.

This is worse than a wasted retry. Rejoining retires the pair at the relay, and
`AccountRelay.retire` closes **both** sockets. So the loop was not merely
failing to connect; it was actively kicking the phone off the relay every
thirty-five seconds, burning its battery and mobile data.

The checklist claimed this loop was already fixed
(`state.take_known_phone_refusal()` sends the relay client to idle). It does —
and then the safety-net timeout returns, `break` falls through to the top, and a
live pairing code re-arms the dial. The wait was treated as permission.

## Root cause 3 — the rule with the worst history was tested by nothing

`desktop/src-tauri/Cargo.toml` sets `[lib] test = false`, and `ws_server.rs` is
not pulled into any integration test via `#[path]` the way `state.rs`,
`heartbeat.rs` and `socket_io.rs` are. So `mod attach_tests` was **compiled by
nothing and run by nothing** — while `docs/behaviour-checklist.md` named it as
the guarantee holding the attach rule in place.

This rule has now regressed three times and reached the user each time. That is
not a coincidence: the safety net named in the checklist did not exist.

## Root cause 4 — the one that was actually dropping the connection

Found by putting the reason into the log and then reading it, live:

```
20:03:51  phone authenticated
20:03:52  notification received app=com.whatsapp stored=true
20:03:52  the phone session ended reason="pending websocket work limit exceeded"
```

`service_while` queues frames that arrive while one application work item is in
flight. `PendingMessages::push_back` refuses past `MAX_PENDING_MESSAGES` (64),
and the refusal was an error, which ended the session.

A phone that reconnects calls `flushPending()` and sends every notification it
was holding. That backlog arrives as one burst, while the first of them is being
written to the database. So the queue filled and the session was killed.

Then it fed itself. The acknowledgements are sent *after* the work completes:

```rust
if let Some(id) = result? {
    send_notification_ack(ws, pairing_key, &id, heartbeat.deadline()).await?;
}
```

`result?` propagates the queue error, so no acknowledgement was sent. The phone
kept the notifications pending, reconnected, and flushed the identical burst,
which killed the session again. Confirmed on the phone at the same moment:

```
W/FocusBridgeSync: could not send a notification; it stays pending
```

This is the original complaint. The interval was never a timer: a session lasted
until the first burst, which is why it was thirty seconds, or two minutes, or
three hours, or instant.

**The fix is back-pressure.** The queue's limit is a memory bound and is
unchanged. What changed is the response to reaching it: `service_while` stops
*reading* the socket while `pending.is_full()`, instead of reading a frame there
is no room for. The frames wait in the socket, TCP closes the phone's window, and
the work item in flight drains the queue. Nothing is dropped.

**Verified live**, on the machine that had been flapping every second: one
authentication, zero session ends, zero queue errors, steady 15-second
heartbeats for over six minutes, and **23 notifications delivered** — the stuck
backlog finally flushed, which is the very burst that used to be fatal.

## Two theories that the evidence killed

Kept here because they were wrong, and the reason they were wrong is the method.

- **"The app inventory exceeds the 1 MB record limit."** The inventory carries a
  base64 PNG icon per app, and the drop was ~1.2 s after every authentication,
  which is when the phone sends it. But the desktop's whole encrypted database is
  569 KB *including* stored icons, and the phone's log never once contained
  `Secure send failed; session discarded`. Not shipped.
- **"A stale LAN attempt tears down the live relay session."** The phone's log
  shows a LAN connection to port 9173 abandoned after 4 s and failing 10 s later,
  about 1.2 s after the relay session authenticated — a very good fit. But
  `finishConnection` is guarded by `connectionSerial`, and every `connect()`
  advances it, so the stale attempt's callback returns without touching the live
  session. The timing was a coincidence.

## Also found, not yet fixed

Neither of these disconnects anything, but both are real and both are visible in
the phone's log on every single connection:

```
--> GET https://192.168.4.23:9173/    ... SocketTimeoutException after 10000ms
--> GET https://172.29.176.1:9173/    ... ConnectException
```

1. **The LAN path never works on this machine.** The desktop listens on
   `0.0.0.0:9173`, and the phone's connection to it times out — an inbound
   firewall rule is missing. Everything is therefore going through the relay,
   which is slower, costs mobile data, and is the only path where the queue burst
   was fatal.
2. **A virtual adapter address is advertised as a candidate.** `172.29.176.1` is
   a WSL/Hyper-V interface that no phone can ever reach. `local_ipv4_candidates`
   should not offer it. It costs a failed attempt on every connection.

Together these add about fourteen seconds to every reconnection.

## The fixes

1. **The two features were split apart.** `auto_first_connection` stays exactly
   what the user means by the toggle: may this PC take a saved phone that turns
   up unasked. `resuming_the_users_connection` is the backend's, always on, and
   covers only the phone that authenticated during this run — `AppState`
   remembers it so it can come back after its transport drops.

   Feature 2 is not a hole in feature 1: it is held in memory only, so a restart
   is still governed by the setting; it has one slot, so connecting a second
   phone ends the first one's standing; and it is cleared in
   `mark_manual_disconnect`, with `may_attach` also returning early when paused —
   two locks on the same door, so Rule 2 is untouched.

2. **A refusal parks this PC until the user asks.** `awaiting_request_after_refusal`
   is set when a phone is turned away and cleared only by
   `request_relay_connection` — which is what picking a saved phone, showing a
   fresh code, and turning automatic reconnection on all go through. The
   safety-net timeout is no longer mistaken for the user asking.

   Trade-off, stated plainly: while parked, this PC is not sitting at the relay.
   A phone that scans a code minted *before* the refusal will not find it. Every
   way of putting a fresh code on screen issues a relay request, so the ordinary
   path is unaffected; the alternative was continuing to kick the phone off
   every thirty-five seconds.

3. **Back-pressure instead of a dropped session** when a burst fills the
   pending queue -- see root cause 4, which is the one that was actually
   disconnecting the phone.

4. **The app writes a log.** `%APPDATA%\com.focusbridge.desktop\logs\focusbridge.log`,
   one rotation at 8 MB, defaulting to `info` instead of to silence, still
   overridable with `RUST_LOG`. And `clear_phone_sender_if_current_with_reason`
   now says why a session ended — the reason was already recorded, but only into
   a diagnostics panel nobody had open at 3am. No call site logs message
   content, capabilities or key material, and none may start.

5. **`may_attach` moved to `focusbridge-core`**, where the crate has no
   `test = false` and its tests actually run. Eleven tests — the two features
   separately, the Rule 2 guard, and the allowance rules — now execute on every
   `cargo test`.

## What was ruled out, with reasons

- **A stale build.** The installed binary is newer than every source change.
- **The heartbeat.** 15s probes, retried, 90s of silence before the session ends,
  inside the 180s advertised in `AUTH_OK`. Sound, and already shipped.
- **The relay's rate limit.** One token per frame, 100-frame burst, one token per
  600 ms. Records chunk at 60 KB and cap at 1 MB, so the largest record is 18
  frames — an order of magnitude inside the burst.
- **The relay's session ceilings.** 24 hours, 1 GB, a million messages. None of
  them lands at thirty seconds or three hours.
- **The phone holding two transports at once.** `SyncEngine` uses one client and
  returns as soon as the LAN path connects, so it is never on both.
- **A plaintext PONG the phone would ignore.** It is encrypted with the pairing
  key, like every other envelope.

## Still open, deliberately

`relay_client::bridge` ends a session after 150 s in which no frame arrived
*from the phone* (`IDLE_TIMEOUT`; only inbound `Binary` resets it). The phone
pings every 15 s, so this needs ten consecutive misses — but 150 s is stricter
than the 180 s this desktop advertises to the phone in `AUTH_OK`, which is the
same class of disagreement the heartbeat fix existed to remove.

It is **not** being changed on suspicion. With logging in place, a session that
ends this way will now say `relay session idle timeout` in the log, and that is
the evidence to act on. If it appears, the fix is to match the advertised
budget.
