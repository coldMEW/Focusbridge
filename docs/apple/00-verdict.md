# The verdict

> Independent-review note (2026-09-07): the outside-EU impossibility and
> Windows ANCS conclusions below are disputed and must not drive implementation.
> Read [the independent review](09-independent-feasibility-review.md) first.
> The original text is preserved for traceability, not endorsed as verified.

You asked whether this project can be made "100% successful on the Apple
environment with no compromises". The honest answer has two halves, and the
second half is not what you were hoping for.

## The short version

| Combination | Possible? | Where the limit comes from |
|---|---|---|
| **Android phone → Mac** | **Yes, fully.** No feature lost. | Nothing. This is ordinary porting work. |
| **iPhone → Mac or Windows PC** | **Yes, but EU-only**, iOS 26.5+, and only through Apple's new accessory framework. | Apple. The framework exists solely to satisfy the EU Digital Markets Act, and Apple gates customer use to EU-located devices with EU Apple Accounts. |
| **iPhone → anything, outside the EU** | **No. Not by any legal route.** | Apple. There is no iOS API that lets an app read other apps' notifications, and the Bluetooth route that once worked was closed in iOS 9. |
| **Mac's own notifications → phone** | Technically yes, but through an undocumented database behind Full Disk Access. Fragile; App Store hostile. | Apple's privacy architecture. Nobody supports this path. |
| **Mac/PC notifications → iPhone** (iPhone as receiver) | Yes, via APNs push. | Different product; costs the end-to-end story unless carefully designed. |

## Read this part carefully

**There is no way to make an iPhone behave like the Android app outside the EU.**
Not with a workaround, not with a clever transport, not with more effort. On
Android, `NotificationListenerService` gives you every notification on the device
after one user grant. iOS has never had an equivalent and still does not. The one
mechanism that ever exposed iPhone notifications to another computer — the Apple
Notification Center Service over Bluetooth — had its consumption removed from iOS
and OS X in iOS 9 **[REPORTED]**; today only dedicated third-party BLE hardware
(watches, fitness bands) can use it, not a Mac and not a PC.

So "no compromises" is not available on this axis. What *is* available is
considerable, and one part of it is better than what the Android app does today.

## The finding that changes the picture

In 2026 Apple shipped two frameworks, **AccessoryTransportExtension** (iOS 26.2)
and **AccessoryNotifications** (iOS 26.5), that do exactly what FocusBridge needs:
they forward *system-wide* iPhone notifications — from apps the user picks — to an
accessory a third party develops, with the content encrypted so that only that
accessory can read it **[VERIFIED]**.

They are not a hack. They are documented, entitled, first-party API, built because
the EU's Digital Markets Act obliged Apple to give third-party devices the access
its own Apple Watch enjoys.

What they deliver per notification is *richer* than the current Android pipeline:
title, subtitle, body, Apple Intelligence summary, the source app's icon, a context
icon, attachments, the app's own action buttons, thread identifier, delivery and
display dates, and priority attributes marking critical / time-sensitive
notifications **[VERIFIED]**. There is a response channel, so a user can dismiss or
quick-reply from the accessory **[VERIFIED]**. That closes the single real gap this
project has against Microsoft Phone Link — notification actions — which
[`../BLUEPRINT.md`](../BLUEPRINT.md) §10 currently lists as outstanding.

And the transport ordering is `bluetooth`, then `localNetwork`, then `internet`
**[VERIFIED]** — which is FocusBridge's own architecture, written down by Apple.

### The price

- **iOS 26.5 or later.** Very new; the installed base in 2026 is small.
- **iPhone only.** Not iPad, not Mac Catalyst, not iOS apps on Apple silicon Macs.
- **EU only for customers.** Apple's exact words: you may develop and test in any
  region, but "customer installations of your app can only use the framework on
  devices located in the EU that are signed in with an Apple Account with an EU
  country or region" **[VERIFIED]**.
- **Three app extensions** plus AccessorySetupKit pairing, three entitlements, and
  a device that is discoverable over Bluetooth LE.
- **Apple Developer Program, $99/year**, unavoidable for any of it **[REPORTED]**.
- Only **one** forwarding target at a time, and turning it on **disables Apple
  Watch notifications** **[REPORTED]**. That is a real thing to tell a user.

### The strategic accident worth noticing

Apple's own **iPhone Mirroring**, which forwards iPhone notifications to a Mac, is
**still unavailable in the EU** as of 2026 **[REPORTED]** — Apple withheld it over
DMA uncertainty. Meanwhile the notification-forwarding framework is **EU-only**.

The two restrictions are exact complements. FocusBridge's iPhone product is legal
precisely where Apple's own answer is missing, and impossible precisely where
Apple already solves it for free. That is a much better position than it first
appears — but it is a *European* product, and it is worth being clear-eyed that
your own machine, on US time, cannot run it in production. Development and testing
work in any region; daily use by you would not.

## Where the architecture already fits

Apple's rules for this framework, in §3.3.3(J) of the Developer Program License
Agreement **[REPORTED]**, require that forwarded notification content:

- must **not** be used for advertising, profiling, model training or location
  monitoring;
- must **not** be disseminated to any other application or device;
- must **not** be stored on cloud servers except as strictly required to deliver
  it to the accessory;
- must be **decrypted only on the accessory itself**.

FocusBridge already satisfies every one of these, by construction. The relay
carries opaque frames it holds no key for, keeps no queue, and stores nothing but
capability hashes; decryption happens only on the paired PC. The project did not
design for this rule and complies with it anyway. That is worth saying in a
submission.

## What to do

1. **Build the Mac desktop app.** It is unblocked, it loses no features, and it
   roughly doubles the addressable users of the Android app. Everything in
   [`02-macos-desktop-port.md`](02-macos-desktop-port.md) is ordinary work.
2. **Prove the iPhone path with a spike before designing anything around it.**
   The single unresolved question is whether a Mac or PC can register as an
   `ASAccessory` — AccessorySetupKit discovers Bluetooth, Wi-Fi-SSID and Wi-Fi
   Aware accessories, and a Mac can advertise Bluetooth LE, but *whether Apple
   intends a general-purpose computer to be an accessory* is unanswered
   **[UNVERIFIED]**. That question decides the entire iPhone product. The
   experiment is in [`08-risk-register.md`](08-risk-register.md) R-01.
3. **Do not build Mac-as-source.** See [`04-macos-as-source.md`](04-macos-as-source.md).
4. **Decide consciously whether a European-only iPhone product is worth it**
   before writing the three extensions. That is a product call, not a technical
   one, and it is yours.

## What "100% with no compromises" actually looks like

Achievable, in full, with no feature lost:

- Android phone → Windows PC (today)
- Android phone → **Mac** (new)

Achievable with the stated gates, and *richer* than today where it works:

- **iPhone → Mac**, EU, iOS 26.5+
- **iPhone → Windows PC**, EU, iOS 26.5+

Not achievable, at any effort:

- iPhone → anything, outside the EU
- Mac's own notifications → anywhere, in a way that would survive review or an
  OS update

Everything after this page is the detail behind those five lines.
