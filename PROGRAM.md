# FocusBridge implementation loop

## Objective

Deliver verified Windows/Android notification sync over LAN/hotspot and a free
authenticated cross-network relay, with protected local storage, authenticated
replay-resistant sessions, explicit device consent, reliable ACK/retry, and
truthful connection state. The Phone Link comparison is in
`docs/phone-link-gap-analysis.md`; it is not a promise of undocumented Microsoft
internals or every privileged OEM feature.

## Mutable Scope

Application sources, shared protocol, isolated tests, build configuration, free
relay tooling/code, and project documentation in this repository. Assign
disjoint files to each agent before work. One desktop Cargo owner and one Android
Gradle owner at a time; shared crypto has its own crate and output directory.

## Immutable Scope

Existing user app data, production key material, unrelated projects, git history,
OS security controls, and paid resources. Preserve all existing working changes.
Never delete/recreate a database to make a migration test pass. Never commit or
push before the user-required release gate. Keep QUERY_ALL_PACKAGES.

## Experiment Unit

One reproducible bug or bounded component: add an executable failing test,
implement, run the test and affected regressions, independently review, update
the evidence record. Continue with the next dependency-ready task. Native
migrations use disposable emulator/temporary database fixtures before live data.

## Validation Commands

- Desktop: MSVC `cargo test --workspace --locked`, `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Frontend: `pnpm exec vitest run`, `pnpm exec tsc --noEmit`, `pnpm build`.
- Android: Gradle 8.7/JDK17 unit tests, lint, debug/test APKs, native instrumentation
  on an explicitly selected disposable emulator, then physical-device acceptance.
- Relay: Worker unit and actual workerd Durable Object/WebSocket tests, quotas,
  ownership/revocation, invalid tokens/roles/frames, restart/hibernation tests.
- Crypto: public Noise vectors, role/identity/PSK/context mismatch, replay,
  reflection, malformed/truncated/oversized input, reconnect/session limits;
  run the same Rust implementation on Windows and Android before app rollout.

## Decision Policy

Use free tiers only. Missing write permissions require browser authorization,
not pasted API tokens. Failed tests block dependent rollout. Compilation is not
device acceptance. A healthy relay socket is not a connected authenticated phone.
Retain LAN operation without an Internet account. Remote access requires explicit
device approval and never bypasses complete disconnect or revocation.

## Escalation Rules

Ask for browser authorization, billing changes, external signing credentials,
destructive data recovery, or actual device actions that cannot safely be
automated. Do not weaken TLS, replay checks, auth, quotas, or test coverage to
get a green result. Never promise access through every administrator firewall.

## Stop Conditions

Finish only when all release gates are evidenced, or report the exact external
blocker while preserving useful tested progress. An interrupted session records
unfinished tasks and commands without claiming success. No automatic paid
upgrade, no unsupervised live database migration, and no production-ready label
based on a subset of tests. Existing release blockers remain visible.
