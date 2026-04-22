# Build Decisions Log

Every deviation from `FocusBridge_Build_Playbook.md` is recorded here.

| Date (UTC) | Step | Deviation | Rationale |
|---|---|---|---|
| 2026-04-22 | §4.2 | Tailwind pinned to v3.4.3 instead of v4 | v4 not GA at pin time; playbook permits |
| 2026-04-22 | §7 (Phase 7) | Out of scope; `FeatureGate` stubbed to always return `false` | Per task directive; AI + payments skipped |
| 2026-04-22 | §1.1 | Android SDK + Docker NOT yet verified at scaffold time | Scaffolding runs before Android/Docker phase gates; installs performed just-in-time for Phase 3 + Phase 5 |
| 2026-04-22 | §1.1 (Rust 1.75.0 pin) | Bumped to Rust stable latest (>= 1.82) | Transitive crates (crypto-common 0.2.1, time 0.3.37+, icu_*, base64ct, idna 1.0, url 2.5.4+) require `edition2024` Cargo feature unavailable in 1.75. Pinning each transitive to older versions produced a runaway chain (>20 crates). Stable >=1.82 supports edition2024 and preserves the Cargo.toml pins; `rust-toolchain.toml` updated accordingly. Safer than a fragile version-pin web. |
