# Build Decisions Log

Every deviation from `FocusBridge_Build_Playbook.md` is recorded here.

| Date (UTC) | Step | Deviation | Rationale |
|---|---|---|---|
| 2026-04-22 | §4.2 | Tailwind pinned to v3.4.3 instead of v4 | v4 not GA at pin time; playbook permits |
| 2026-04-22 | §7 (Phase 7) | Out of scope; `FeatureGate` stubbed to always return `false` | Per task directive; AI + payments skipped |
| 2026-04-22 | §1.1 | Android SDK + Docker NOT yet verified at scaffold time | Scaffolding runs before Android/Docker phase gates; installs performed just-in-time for Phase 3 + Phase 5 |
| 2026-04-22 | §1.1 (Rust 1.75.0 pin) | Bumped to Rust stable latest (>= 1.82) | Transitive crates (crypto-common 0.2.1, time 0.3.37+, icu_*, base64ct, idna 1.0, url 2.5.4+) require `edition2024` Cargo feature unavailable in 1.75. Pinning each transitive to older versions produced a runaway chain (>20 crates). Stable >=1.82 supports edition2024 and preserves the Cargo.toml pins; `rust-toolchain.toml` updated accordingly. Safer than a fragile version-pin web. |
| 2026-04-22 | §1.1 Rust target | `x86_64-pc-windows-gnu` instead of MSVC | No MSVC toolchain present; MinGW-w64 g++ already on PATH. |
| 2026-04-22 | §1.1 Tauri 2.0.0-beta.22 | Upgraded to Tauri v2 GA (`2.x`) for CLI + runtime + plugins | Playbook §1.1 explicitly permits "GA 2.x if released". Beta-22 ecosystem is stale; GA preserves the API contract this project uses. Affects `desktop/package.json` and `desktop/src-tauri/Cargo.toml`. |
| 2026-05-03 | §4/§5 local transport | Desktop MVP WebSocket listener currently uses plain WS on `:9173`; WSS hardening remains pending | This creates a working end-to-end Android-to-desktop path first. Self-signed TLS generation/fingerprint plumbing exists, but certificate persistence + Android pinning need a focused follow-up. |
| 2026-05-03 | §5.2 Kotlin Compose plugin | Android uses Kotlin 1.9.24 with Compose compiler extension instead of `org.jetbrains.kotlin.plugin.compose` | The Compose compiler Gradle plugin is a Kotlin 2.x workflow. For the pinned Kotlin 1.9.24 stack, `composeOptions.kotlinCompilerExtensionVersion` is the compatible build path. |
