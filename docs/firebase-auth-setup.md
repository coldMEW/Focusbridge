# Firebase Auth Setup

FocusBridge now uses Firebase client authentication for email/password login and signup on desktop. The local FocusBridge password/PIN remains the device app-lock.

## Firebase Console

1. Open Firebase Console for project `foucsbridge`.
2. Go to Authentication > Sign-in method.
3. Enable Email/Password.
4. Keep the Android app package name as `com.focusbridge.android`.
5. Keep the Web app config matching the desktop config in `desktop/src/lib/firebaseAuth.ts`.

## Android

The Android config file belongs at:

```text
android/app/google-services.json
```

Android uses:

- Google Services Gradle plugin `4.4.4`
- Firebase BoM `33.7.0`
- Firebase Auth

The BoM is pinned below Firebase `34.12.0` because this project currently uses Kotlin `1.9.24`; Firebase BoM `34.12.0` pulls artifacts compiled with Kotlin 2.x metadata and breaks the current Android build.

## Desktop

Desktop uses the Firebase Web SDK through:

```text
desktop/src/lib/firebaseAuth.ts
```

The Firebase web config values are public client identifiers, not server secrets. For production, prefer setting them through environment variables before build:

```env
VITE_FIREBASE_API_KEY=...
VITE_FIREBASE_AUTH_DOMAIN=...
VITE_FIREBASE_PROJECT_ID=...
VITE_FIREBASE_STORAGE_BUCKET=...
VITE_FIREBASE_MESSAGING_SENDER_ID=...
VITE_FIREBASE_APP_ID=...
VITE_FIREBASE_MEASUREMENT_ID=...
```

## Current Boundary

Firebase login/signup authenticates the desktop user. LAN/hotspot sync still pairs Android and desktop with the local QR pairing key. Relay v1.1 should verify Firebase ID tokens server-side before issuing relay sessions.
