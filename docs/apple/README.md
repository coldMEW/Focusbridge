# FocusBridge on Apple platforms — research folder

> Independent-review note (2026-09-07): start with
> [the independent feasibility review](09-independent-feasibility-review.md).
> It corrects overly broad impossibility claims in the original research.
> [Change record](13-change-record.md) documents scope and rollback.

## Current handoff (2026-09-08)

See [competitor mechanisms](17-competitor-mechanisms.md) for Phone Link, Unison,
Dell Mobile Connect, KDE Connect, Garmin, Pushover, Pushcut, AirDroid and Apple's
own forwarding, including what their features do not establish for FocusBridge.

New evidence: [Shortcuts notification-source investigation](16-shortcuts-source-investigation.md)
corrects the earlier blanket rejection of notification automations. It is a
documented trigger and an untested FocusBridge ingestion candidate, not full iOS parity.

Start with [decision and execution order](15-decision-and-execution-order.md).
Independent reports cover [accessory APIs](10-accessory-framework-verification.md),
[macOS porting](11-macos-port-verification.md),
[current security findings](12-security-review-scope.md), and
[budget/privacy/test gates](14-budget-privacy-and-test-gates.md).
These reports supersede conflicting conclusions below, not the need for hardware tests.

Everything learned about taking FocusBridge to macOS and iPhone: what is possible,
what is not, what it costs, and what to build in which order.

Research date: **2026-09-07**. Written against commit `cdf418f`.

## Read in this order

| Document | What it answers |
|---|---|
| [`00-verdict.md`](00-verdict.md) | The short answer. Read this first, and read all of it. |
| [`01-iphone-as-source.md`](01-iphone-as-source.md) | Can an iPhone capture its own notifications and send them out? The crux of the whole expansion. |
| [`02-macos-desktop-port.md`](02-macos-desktop-port.md) | Porting the Tauri desktop app to macOS. The lowest-risk, highest-value work. |
| [`03-ios-app-engineering.md`](03-ios-app-engineering.md) | The iPhone app itself: stack, the Rust core, TLS pinning, background limits, storage. |
| [`04-macos-as-source.md`](04-macos-as-source.md) | Can a Mac's own notifications be mirrored elsewhere? |
| [`05-shared-core-and-protocol.md`](05-shared-core-and-protocol.md) | One Rust engine across four platforms; protocol and QR changes; capability negotiation. |
| [`06-distribution-and-cost.md`](06-distribution-and-cost.md) | Signing, notarisation, App Store, and the money. |
| [`07-roadmap.md`](07-roadmap.md) | Phased plan with gates and exit criteria. |
| [`08-risk-register.md`](08-risk-register.md) | Every risk, every open question, and exactly how to settle each. |
| [`sources.md`](sources.md) | Every URL this research rests on. |

## How this research was done, and how far to trust it

Primary sources first. The findings that decide the project come from Apple's own
documentation, fetched as structured JSON from
`developer.apple.com/tutorials/data/documentation/*.json` — the same content the
developer site renders — so framework names, availability annotations, entitlement
identifiers and the regional restrictions are quoted, not paraphrased from a blog.

Secondary sources (MacRumors, 9to5Mac, Apple Developer Forums, the Tauri docs)
are used for release timing, policy reporting and community experience, and are
labelled as such.

Every claim in these documents carries one of three confidence markers:

- **[VERIFIED]** — read directly from Apple documentation or another primary
  source, with the URL in `sources.md`.
- **[REPORTED]** — from credible secondary reporting; consistent across sources
  but not confirmed against a primary one.
- **[UNVERIFIED]** — reasoning or inference. Must be settled by experiment before
  anything is built on it. Every one of these appears in `08-risk-register.md`
  with the experiment that settles it.

Nothing here has been run on real hardware. No Apple device was available during
this research and no code was compiled. Treat the whole folder as a plan to be
tested, not a result. That is this project's rule — see
[`../behaviour-checklist.md`](../behaviour-checklist.md) — and it applies to
research too.

## The one-paragraph summary

A **macOS desktop app is straightforwardly achievable** and is the obvious first
move. An **iPhone as a notification source is possible**, but only through a
framework Apple shipped in 2026 under EU regulatory pressure — it is iPhone-only,
requires iOS 26.5, and **works for customers only in the EU**. Outside the EU
there is no legal route at all, and Apple's own iPhone Mirroring fills the gap
instead — except in the EU, where Apple does not ship it. The strategic shape is
unusually clean: FocusBridge's iPhone story is viable precisely and only where
Apple's own answer is absent.
