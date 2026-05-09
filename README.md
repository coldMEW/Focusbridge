# FocusBridge

FocusBridge is a local-first Android and desktop app for routing phone notifications to a desktop workspace. It is built for users who want phone awareness without keeping the phone open.

The v1 desktop app runs a local secure WebSocket server. The Android app captures notifications through Android notification access, filters them, and syncs them to the desktop over LAN or hotspot pairing.

## Features

- Android notification capture and desktop delivery
- QR and manual pairing
- Local LAN/hotspot sync
- WSS transport with certificate pinning
- Message-level encrypted envelopes
- Heartbeat and reconnect handling
- Notification ACK and retry queue
- Masked Peek privacy mode
- Study Mode filtering
- Priority, blocked keyword, contact, and app rules
- Desktop app control for synced Android apps
- Native desktop notifications
- Tray/background behavior
- Firebase email/password account entry plus local app lock

## Repository

```text
android/        Android app, Kotlin, Compose, Room, Hilt
desktop/        Desktop app, Tauri, Rust, React
relay/          Optional relay service planned for v1.1 cross-network sync
shared/         Shared protocol reference
docs/           Design notes, release checklist, project memory
```

## Requirements

- Windows 10/11 for the primary desktop build
- Node.js and pnpm
- Rust stable with the MSVC toolchain
- Visual Studio C++ Build Tools
- Android Studio or Android SDK with JDK 17
- Android phone running API 26 or newer

## Development

Desktop:

```powershell
cd desktop
pnpm install
pnpm tauri dev
```

Android:

```powershell
cd android
.\gradlew.bat assembleDebug
```

Relay:

```powershell
cd relay
cargo test
```

## Production Builds

Desktop installer:

```powershell
cd desktop
pnpm install
pnpm tauri build --ci
```

Android APK:

```powershell
cd android
.\gradlew.bat assembleRelease
```

Current Android release signing uses the debug signing config for local testing. Replace it with a real release keystore before public distribution.

## Reset Local Data

Desktop app data is stored under:

```text
%APPDATA%\com.focusbridge.desktop
%LOCALAPPDATA%\com.focusbridge.desktop
```

Android app data can be reset from a machine with `adb`:

```powershell
adb shell pm clear com.focusbridge.android
```

## Privacy

FocusBridge is designed around local-first sync. Notification content is stored locally in the desktop and Android app databases. Relay support is planned as optional infrastructure and should only carry private data after the encrypted sync path is enabled for that mode.

## License

AGPL-3.0. See [LICENSE](LICENSE).
