# Build Decisions Log

Every deviation from `FocusBridge_Build_Playbook.md` is recorded here.

| Date (UTC) | Step | Deviation | Rationale |
|---|---|---|---|
| 2026-04-22 | §4.2 | Tailwind pinned to v3.4.3 instead of v4 | v4 not GA at pin time; playbook permits |
| 2026-04-22 | §7 (Phase 7) | Out of scope; `FeatureGate` stubbed to always return `false` | Per task directive; AI + payments skipped |
| 2026-04-22 | §1.1 | Android SDK + Docker NOT yet verified at scaffold time | Scaffolding runs before Android/Docker phase gates; installs performed just-in-time for Phase 3 + Phase 5 |
