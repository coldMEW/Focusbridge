# FocusBridge v1.0 Release Checklist

FocusBridge v1.0 is the LAN/hotspot release for Windows desktop plus Android phone. Different-network sync belongs to v1.1 relay mode.

## Build Commands

Android verification and test APK:

```powershell
cd android
cmd /c "set GRADLE_USER_HOME=C:\Users\DSU\.gradle&& gradlew.bat testDebugUnitTest lint assembleDebug assembleRelease"
```

Desktop verification and Windows installer:

```powershell
cd desktop
pnpm install
pnpm tsc --noEmit
pnpm vitest run
pnpm build
pnpm tauri build --ci
```

## Artifact Paths

- Android debug APK: `android/app/build/outputs/apk/debug/app-debug.apk`
- Android release APK: `android/app/build/outputs/apk/release/app-release.apk`
- Windows installer: `desktop/src-tauri/target/release/bundle/msi/`
- Windows executable: `desktop/src-tauri/target/release/focusbridge-desktop.exe`

## Windows Smoke Test

1. Install or run the packaged FocusBridge desktop build, not `pnpm tauri dev`.
2. Open Settings and run `Allow FocusBridge through Windows Firewall`.
3. Confirm Diagnostics shows port `9173` and at least one LAN/hotspot endpoint.
4. Confirm the tray icon shows the FocusBridge logo.
5. Minimize or run in tray, send a phone notification, and confirm the native toast says FocusBridge.

Dev note: Windows can show PowerShell/dev-shell identity for toasts launched through `pnpm tauri dev`. Release acceptance must use the packaged installer/build because Windows toast identity is tied to AppUserModelID and installed shortcut metadata.

## Android Smoke Test

1. Install the APK on a physical Android phone.
2. Open FocusBridge and complete the Production setup checklist.
3. Grant notification listener access.
4. Allow camera access and scan the desktop QR.
5. If sync drops after idle time, open Android battery settings from the checklist and disable battery optimization for FocusBridge.
6. Send a notification from WhatsApp, Messages, Gmail, or another app and confirm it appears on desktop.

## Call Notifications

FocusBridge captures Android notifications. Incoming phone/social-media calls are shown on desktop when the app exposes the call as a notification visible to Android's Notification Listener. v1.0 does not hook the raw Telecom/call-state API.

## Release Signing

The current local `release` APK uses the debug signing config so it is installable for testing. Before public distribution, replace this with a private release keystore and do not commit the keystore or passwords.
