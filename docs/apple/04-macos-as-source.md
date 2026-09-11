# Can a Mac's own notifications be mirrored elsewhere?

The symmetric question to the iPhone one. Short answer: **technically yes, in a way
that is undocumented, permission-heavy and fragile. Do not build it.**

Included because it will be proposed, and because the reason to reject it is worth
having written down.

---

## 1. What exists

### The Notification Center database

macOS keeps delivered notifications in a SQLite database at
`~/Library/Group Containers/group.com.apple.usernoted/db2/db` **[REPORTED]**. It
holds the notification title, subtitle, body, the delivering application's bundle
identifier, the delivery timestamp, and whether the user interacted with it — in
plaintext.

Reading it requires **Full Disk Access** **[REPORTED]**. macOS Sequoia moved the
database into Group Containers explicitly as a hardening measure, after security
researchers publicised how readable it was.

This is the mechanism every "read Mac notifications" tool uses. It works today.

### Everything else

| Approach | Verdict |
|---|---|
| Accessibility API (`AXUIElement`) scraping notification banners | Requires the Accessibility permission, breaks with UI changes, and depends on a banner being on screen — misses anything delivered quietly. Worse than the database on every axis. |
| `NSDistributedNotificationCenter` | Carries app-defined messages between processes. It is not the user-facing notification system and cannot observe it. |
| `NSUserNotificationCenter` / `UNUserNotificationCenter` | Own app only, same as iOS. |
| Screen Time / `DeviceActivity` on macOS | Usage reporting, not content. |
| A macOS equivalent of `AccessoryNotifications` | **Does not exist.** The framework is iPhone-only **[VERIFIED]**. |

---

## 2. Why not to build it

1. **Full Disk Access is a hostile ask.** It is the broadest permission macOS
   grants — every file the user owns, including Mail, Messages and browser data.
   Asking for it in order to mirror notifications is disproportionate, and a
   privacy-first product asking for it undercuts its own pitch. FocusBridge's
   README says "notification content is readable by exactly two machines"; an app
   that reads everything on disk sits badly next to that sentence.
2. **It is undocumented and unstable.** The path has moved at least once (Sequoia),
   the schema is not a contract, and Apple has already demonstrated willingness to
   tighten it. A feature built on it will break on an OS update with no warning and
   no migration path.
3. **The App Store is closed to it**, and even notarised distribution would carry
   an unexplainable permission prompt.
4. **Polling, not events.** There is no change notification for the database, so
   the design becomes a poller — latency and battery cost for a worse result than
   the sub-second path the Android side achieves.
5. **The demand is unclear.** FocusBridge exists because the phone is the thing you
   pick up and lose twenty minutes to. A Mac's notifications are already on the
   screen you are looking at.

---

## 3. What Apple and others already ship

This is the competitive context that makes the whole Mac-as-source question moot,
and it also bears on the iPhone story.

| Product / feature | Mechanism | Cross-network? | Notes |
|---|---|---|---|
| **Apple iPhone Mirroring** (macOS 15+ / iOS 18+) | Continuity, proximity-bound | No | Forwards iPhone notifications to the Mac. **Still unavailable in the EU** as of 2026 — Apple withheld it over DMA uncertainty **[REPORTED]** |
| **Apple Watch notifications** | Continuity, not third-party ANCS | No | Mutually exclusive with third-party notification forwarding **[REPORTED]** |
| **iOS 26.3+ Notification Forwarding** | `AccessoryNotifications` | Transport enum includes `internet` | **EU only** **[VERIFIED]** |
| **Microsoft Phone Link (iPhone)** | Bluetooth | No | Thin compared with its Android support **[REPORTED]** |
| **Microsoft Phone Link (Android)** | Wi-Fi + Bluetooth; "sync over mobile data" is an opt-in the docs recommend leaving off | Barely, and off by default | Already analysed in [`../phone-link-gap-analysis.md`](../phone-link-gap-analysis.md) |
| **Intel Unison / Dell Mobile Connect** | Bluetooth | No | Unison narrow; DMC discontinued **[REPORTED]** |
| **Android → Mac** | KDE Connect and similar | LAN only | This is the gap a FocusBridge Mac build fills |

Two conclusions fall out of that table.

**First**, the Mac build is genuinely differentiated. Nothing mirrors Android
notifications to a Mac across networks. KDE Connect is LAN-only. Phone Link does
not exist for macOS. Building
[`02-macos-desktop-port.md`](02-macos-desktop-port.md) puts FocusBridge somewhere
nobody else is.

**Second**, the iPhone restriction and Apple's own restriction are exact
complements: Apple does not ship iPhone Mirroring in the EU, and the EU is the only
place third-party notification forwarding works. FocusBridge's iPhone product is
legal exactly where Apple's answer is absent — and redundant exactly where it is
present.

---

## 4. If it were ever built anyway

For the record, the shape it would have to take:

- Direct distribution only, never the App Store.
- Full Disk Access requested with a plain explanation, and the app must work
  usefully without it rather than being a dead shell.
- A version-pinned schema reader that **refuses to guess** when it meets a database
  it does not recognise — the same principle the encrypted-database migrator
  already follows (`ab9b7de`): fail closed, never silently produce wrong results.
- The behaviour checklist would need a row stating plainly that this path depends
  on an undocumented file and may stop working on any macOS update.

That is a lot of machinery for a feature nobody asked for. Recommendation stands:
**do not build it.**
