# Sources

Everything this folder rests on, gathered 2026-09-07.

Apple's documentation site renders from JSON at
`https://developer.apple.com/tutorials/data/documentation/<path>.json`. The pages
below were read that way, so availability annotations, entitlement identifiers and
the regional restrictions are quoted from Apple's own data rather than from a
summary. Anything marked **[VERIFIED]** in the other documents comes from this
first section.

## Apple — primary

### Notification forwarding to third-party accessories

- Accessory Notifications (framework) — <https://developer.apple.com/documentation/accessorynotifications>
  — iOS 26.5. Source of the "iPhone only" and EU-customer restrictions, and of the
  extension model description.
- `NotificationsForwarding` — <https://developer.apple.com/documentation/accessorynotifications/notificationsforwarding>
- `AccessoryNotification` — <https://developer.apple.com/documentation/accessorynotifications/accessorynotification>
  — the full field list: title, subtitle, body, summary, sourceIcon, contextIcon,
  attachments, actions, identifier, sourceName, threadIdentifier, deliveryDate,
  displayDate, attributes.
- `AccessoryNotificationCenter` — <https://developer.apple.com/documentation/accessorynotifications/accessorynotificationcenter>
  — `requestForwarding(for:)`, `forwardingStatus(for:)`, `presentSettings(for:scenePersistentIdentifier:)`.
- `ForwardingDecision` — <https://developer.apple.com/documentation/accessorynotifications/forwardingdecision>
  — `.allow`, `.limited`, `.deny`, `.undetermined`.
- `AlertingContext` — <https://developer.apple.com/documentation/accessorynotifications/alertingcontext>
  — `shouldAlert`, `notificationCanAlert`, `isSuppressedByFocus`, `kind`, `sound`.
- Accessory Transport Extension (framework) — <https://developer.apple.com/documentation/accessorytransportextension>
  — iOS 26.2. The three-extension architecture and the "available only for iOS"
  restriction.
- **Receiving iOS notifications on an accessory** — <https://developer.apple.com/documentation/accessorytransportextension/receiving-ios-notifications-on-an-accessory>
  — the most important single page in this research. Full code samples, the
  `EXExtensionPointIdentifier` values, and the entitlement requirements.
- `AccessoryTransport` — <https://developer.apple.com/documentation/accessorytransportextension/accessorytransport>
  — iOS 26.5. `bluetooth`, `localNetwork`, `internet`, and the selection order.
- `AccessoryMessage` — <https://developer.apple.com/documentation/accessorytransportextension/accessorymessage>
- `AccessoryTransportAppExtension` — <https://developer.apple.com/documentation/accessorytransportextension/accessorytransportappextension>
  — `dataEventHandler`, `.ciphertext` / `.plaintext`, the
  `com.apple.developer.accessory-transport-extension` entitlement.
- `AccessoryTransportSecurity` — <https://developer.apple.com/documentation/accessorytransportextension/accessorytransportsecurity>
  — the `com.apple.developer.accessory-transport-security` entitlement.
- `com.apple.developer.accessory-data-provider` — <https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.accessory-data-provider>

### Pairing and discovery

- AccessorySetupKit — <https://developer.apple.com/documentation/accessorysetupkit/>
  — Bluetooth and Wi-Fi discovery, the Info.plist keys, and the watchOS 26 note
  about companion apps reaching ASK accessories over CoreBluetooth.
- `ASDiscoveryDescriptor` — <https://developer.apple.com/documentation/accessorysetupkit/asdiscoverydescriptor>
  — every discovery trait: Bluetooth service UUID, company identifier,
  manufacturer/service data blob and mask, name substring, range; Wi-Fi SSID and
  prefix; Wi-Fi Aware service name, role, model and vendor.
- `ASAccessory` — <https://developer.apple.com/documentation/accessorysetupkit/asaccessory>
- Wi-Fi Aware — <https://developer.apple.com/documentation/wifiaware>
  — iOS/iPadOS/Mac Catalyst 26.0, iPhone 12 and later. **No macOS row**, which is
  what rules a Mac out as a Wi-Fi Aware accessory.

### Other Apple documentation

- Apple Notification Center Service specification — <https://developer.apple.com/library/archive/documentation/CoreBluetooth/Reference/AppleNotificationCenterServiceSpecification/Specification/Specification.html>
  — the GATT characteristics, their UUIDs, mandatory vs optional support, and the
  warning that ANCS is not guaranteed to be present.
- `UNNotificationServiceExtension` — <https://developer.apple.com/documentation/usernotifications/unnotificationserviceextension>
  — including Apple's own statement that it can be used "to decrypt an encrypted
  data block", which is what makes the iPhone-as-receiver design possible without
  giving plaintext to APNs.
- Security of runtime process (App Sandbox) — <https://support.apple.com/guide/security/security-of-runtime-process-sec15bfe098e/web>
- Protecting user data with App Sandbox — <https://developer.apple.com/documentation/security/protecting-user-data-with-app-sandbox>
- iPhone Mirroring — <https://support.apple.com/en-us/120421>

### Apple Developer Forums

- ANCS service not discoverable from OS X — <https://developer.apple.com/forums/thread/24336>
- ANCS client help — <https://developer.apple.com/forums/thread/764443>
- Intercepting system-wide notifications on macOS — <https://developer.apple.com/forums/thread/758451>
- Local Network permission ignored after reboot — <https://developer.apple.com/forums/thread/792453>
- `sendto` fails under Sequoia's LAN privacy — <https://developer.apple.com/forums/thread/765285>
- Cannot reach local network devices when the app is not in `/Applications` — <https://developer.apple.com/forums/thread/759262>
- No local-network prompt on Sequoia/Tahoe — <https://developer.apple.com/forums/thread/814226>

## Secondary — release timing, policy and community experience

Used for **[REPORTED]** claims.

### The DMA changes

- iOS 26.3 adds Notification Forwarding for third-party wearables — <https://www.macrumors.com/2025/12/15/ios-26-3-notification-forwarding/>
- EU iPhone users get AirPods-like pairing and notification forwarding in iOS 26.5 — <https://www.macrumors.com/2026/05/11/ios-26-5-eu-third-party-wearable-changes/>
- iOS 26.5 features — <https://www.macrumors.com/2026/05/11/ios-26-5-features/>
- Apple sets privacy rules for third-party access to Live Activities and notifications — <https://www.macrumors.com/2026/03/31/apple-sets-privacy-rules-live-activities-alerts/>
- Apple introduces privacy rules for third-party access to notifications — <https://9to5mac.com/2026/03/30/apple-introduces-privacy-rules-for-third-party-access-to-notifications-and-live-activities/>
  — the source for §3.3.3(J) of the Developer Program License Agreement: no
  advertising, profiling, model training or location monitoring; no dissemination
  to other applications or devices; no cloud storage beyond delivery; decryption
  only on the accessory.
- New in iOS 26.3 — <https://appleinsider.com/articles/25/12/15/new-in-ios-263-android-transfer-settings-third-party-notification-forwarding>
- iOS 26.3 proximity pairing for third-party devices in the EU — <https://www.engadget.com/mobile/apples-ios-263-will-introduce-proximity-pairing-to-third-party-devices-in-the-eu-133037696.html>
- iOS 26.3 will come with Notification Forwarding for third-party accessories — <https://appleosophy.com/2025/12/17/ios-26-3-will-come-with-notification-forwarding-for-third-party-accessories/>
- European Commission welcomes Apple's iOS interoperability changes — <https://cadeproject.org/updates/commission-welcomes-apples-ios-interoperability-changes/>

### iPhone Mirroring and the EU

- iPhone mirroring on the Mac — better, but without the EU — <https://www.heise.de/en/news/iPhone-mirroring-on-the-Mac-even-better-but-without-the-EU-10670669.html>
- Apple's iPhone Mirroring remains unavailable in the EU — <https://themunicheye.com/apple-iphone-mirroring-eu-unavailable-23405>

### ANCS on macOS

- Intercepting iOS notifications — <https://www.appfoundry.be/blog/apple-notification-center-service>
- Forwarding iOS notifications to any device capable of receiving them — <https://medium.com/@tombastable/forwarding-ios-notifications-to-any-device-capable-of-receiving-notifications-6735b1ac5451>
  — the source for ANCS consumption having been removed from iOS and OS X in iOS 9.
- `INDANCSClient` — <https://github.com/indragiek/INDANCSClient> — a historical
  Objective-C ANCS client, useful as evidence of what used to be possible.
- Nordic nRF Connect SDK ANCS client — <https://nrfconnectdocs.nordicsemi.com/ncs/2.5.0/nrf/libraries/bluetooth_services/services/ancs_client.html>
- Silicon Labs ANCS application note — <https://docs.silabs.com/bluetooth/2.13/bluetooth-code-examples-applications/apple-notification-center-service>

### macOS local network privacy

- Local Network Privacy on Sequoia — <https://mjtsai.com/blog/2024/10/02/local-network-privacy-on-sequoia/>
- Manage privacy protection for network devices — <https://eclecticlight.co/2025/03/10/manage-privacy-protection-for-network-devices-and-others/>

### macOS Notification Center database

- Apple addresses privacy concerns around the Notification Center database in macOS Sequoia — <https://9to5mac.com/2024/09/01/security-bite-apple-addresses-privacy-concerns-around-notification-center-database-in-macos-sequoia/>
- The 'dark' side of macOS notifications — <https://objective-see.org/blog/blog_0x2E.html>
- macOS incident response: user data, activity and behaviour — <https://www.sentinelone.com/labs/macos-incident-response-part-2-user-data-activity-and-behavior/>

### Tauri and macOS distribution

- macOS Code Signing — <https://v2.tauri.app/distribute/sign/macos/>
- Tauri 2.0 — <https://v2.tauri.app/>
- Shipping a production macOS app with Tauri 2.0 — <https://dev.to/0xmassi/shipping-a-production-macos-app-with-tauri-20-code-signing-notarization-and-homebrew-mc3>
- Ship your Tauri v2 app: code signing for macOS and Windows — <https://dev.to/tomtomdu73/ship-your-tauri-v2-app-like-a-pro-code-signing-for-macos-and-windows-part-12-3o9n>
- A practical guide to packaging, signing and notarizing macOS apps — <https://www.ubitools.com/macos-app-packaging-signing-notarization/>

## What was not consulted

Stated so the gaps are known:

- No WWDC session videos were watched; the frameworks are new enough that session
  coverage may exist and would be worth finding.
- The Developer Program License Agreement itself was not read directly — §3.3.3(J)
  is quoted via reporting. **Read the actual agreement before submission.**
- No Apple hardware was available, so nothing here was executed, compiled or
  measured.
- Apple's Accessory Design Guidelines PDF, referenced by the Wi-Fi Aware
  documentation, was not retrieved and may contain accessory requirements relevant
  to R-01.
