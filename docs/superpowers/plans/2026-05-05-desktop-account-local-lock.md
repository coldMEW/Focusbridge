# Desktop Account Local Lock Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the production desktop auth flow: Firebase login/signup/guest account gate, 90-day account session, local PIN/password setup, security-question recovery, and polished UI.

**Architecture:** Keep Firebase account behavior in `desktop/src/lib/firebaseAuth.ts`, session expiry logic in a small tested frontend utility, and local lock storage/verification in Rust Tauri commands. `AuthGate.tsx` orchestrates the two-stage flow and remains the only dashboard gate.

**Tech Stack:** React, TypeScript, Firebase Auth, Tauri commands, Rust PBKDF2, Vitest.

---

### Task 1: Account Session Utility

**Files:**
- Create: `desktop/src/lib/accountSession.ts`
- Create: `desktop/src/lib/accountSession.test.ts`

- [ ] Write failing tests for 90-day validity, expired sessions, guest sessions, and missing sessions.
- [ ] Implement `ACCOUNT_SESSION_DAYS`, `accountSessionValid`, `accountSessionExpired`, and `accountSessionLabel`.
- [ ] Run `pnpm vitest run src/lib/accountSession.test.ts`.

### Task 2: Firebase Forgot Password

**Files:**
- Modify: `desktop/src/lib/firebaseAuth.ts`

- [ ] Add `firebaseSendPasswordReset(email)`.
- [ ] Surface Firebase reset errors through the existing error formatter in `AuthGate.tsx`.

### Task 3: Local Security Question Commands

**Files:**
- Modify: `desktop/src-tauri/src/commands/auth_cmd.rs`
- Modify: `desktop/src-tauri/src/lib.rs`

- [ ] Add `auth_register_with_recovery(password, security_question, security_answer)`.
- [ ] Add `auth_recovery_question()`.
- [ ] Add `auth_reset_password_with_recovery(security_answer, new_password)`.
- [ ] Reuse PBKDF2 with separate salts for password and security answer.
- [ ] Normalize security answers before hashing.

### Task 4: AuthGate UI

**Files:**
- Modify: `desktop/src/components/AuthGate.tsx`
- Modify: `desktop/src/index.css`

- [ ] Replace single-card auth with account-first and local-lock second stage.
- [ ] Add Login, Sign up, Guest, Forgot password, local recovery, security-question setup, and reset states.
- [ ] Add branded animation and better copy.
- [ ] Preserve advanced relay auth as collapsed secondary functionality.

### Task 5: Verification

**Commands:**
- `pnpm vitest run`
- `pnpm build`
- `cargo check --no-default-features`

- [ ] Fix failures without weakening the auth behavior.
- [ ] Commit and push.
