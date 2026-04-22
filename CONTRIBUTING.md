# Contributing to FocusBridge

Thanks for your interest.

## License

This project is AGPL-3.0. By submitting a patch you agree your contribution is licensed under AGPL-3.0.

## Tests

Every patch MUST include tests for changed logic:

- Android: JUnit / MockK unit tests in `android/app/src/test/`. Run `./gradlew test`.
- Desktop: Rust `cargo test` + frontend `pnpm vitest run`.
- Relay: `cargo test`.

All three CI workflows must stay green.

## Per-App Notification Parsers

Community parsers live in `android/app/src/main/java/com/focusbridge/android/processor/parsers/`.

To contribute a parser:

1. Implement `NotificationParser`.
2. Register it in `ParserRegistry`.
3. Include mock `StatusBarNotification` tests.
4. Open a PR.

## Style

- Kotlin: official style.
- Rust: `cargo fmt` + `cargo clippy -- -D warnings` must pass.
- TypeScript: project Prettier/ESLint config.

No version bumps without discussion. Versions are pinned intentionally — see `FocusBridge_Build_Playbook.md` §1.1.
