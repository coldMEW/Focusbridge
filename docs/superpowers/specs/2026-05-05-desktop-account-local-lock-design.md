# Desktop Account and Local Lock Design

## Goal

FocusBridge desktop opens with account sign-in or guest mode, then requires the local PIN/password vault lock before showing the dashboard. Firebase identity is used for user account ownership and future settings sync, while the local lock protects this desktop install.

## Flow

1. Account gate appears first with Login, Sign up, and Guest choices.
2. Login checks Firebase Email/Password. Missing users receive the Firebase error and can switch to Sign up.
3. Sign up creates the Firebase user. Existing users receive the Firebase error and can switch to Login.
4. Guest mode skips Firebase and stores settings locally only.
5. Firebase/guest account state is accepted for 90 days. After 90 days the account gate is shown again.
6. After account selection, the local vault lock appears.
7. First local setup requires a PIN/password plus a security question and answer.
8. Returning users unlock with PIN/password.
9. Forgot Firebase password sends a Firebase password reset email.
10. Forgot local PIN/password asks the configured security question. A correct answer allows the user to set a new PIN/password.

## Security

Firebase passwords are handled only by Firebase Auth. Local PIN/password and security answers are hashed in Rust with PBKDF2 and random salts. Security answers are normalized by trimming, lowercasing, and collapsing whitespace before hashing so small typing differences do not block recovery.

## UX

The auth screen uses a branded two-step layout with the FocusBridge logo, smooth card entrance, clear step status, and large primary actions. Advanced relay auth remains available but collapsed because it is not the primary v1.0 path.

## Scope Boundary

This spec implements the desktop auth gate. Android account UI is intentionally left for a separate Android-auth change because the current verified Android sync path should not be destabilized by adding a new first-run gate in the same patch.
