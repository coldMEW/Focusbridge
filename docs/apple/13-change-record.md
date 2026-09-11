# Research change record

Baseline: HEAD `cdf418f`, 2026-09-07. Existing dirty README.md,
docs/PROJECT_MEMORY.md, docs/BLUEPRINT.md and docs/apple belong to the user.

## Authorized research changes

- Add independent feasibility, accessory, macOS and scoped security reviews.
- Add an editorial warning above the original verdict and index, leaving the
  historical text intact. Reason: outside-EU impossibility claim is contradicted
  by Microsoft's documented iPhone notification feature.
- No application, database, signing, deployment or dependency changes in this
  research step. No source code is reverted to an older assistant checkpoint.

Rollback: remove only the new independent review files and the explicitly
  marked independent-review note in the two original documents. Do not remove
  the pre-existing apple directory or overwrite unrelated user edits.

Verification: check relative Markdown links and git diff whitespace; independent
  source checks are documented in the reviews. Apple builds/hardware tests are
  not performed. Any future code fix requires its own pre-edit entry with exact
  paths, tests and rollback procedure.

## Continuation 2026-09-08

Competitor research addition: document 17 and its README index link. This
documents mechanisms and test gates only. Reverse by removing those additions;
no app source, data or vendor account changes were performed.

Additional research: document 16 and its index links record newly found Apple
WWDC26 notification-automation evidence. No application changes. Remove only
this report and associated index links to reverse that documentation addition.

The three interrupted reviewers saved documents 10, 11 and 12 before stopping.
Parent rechecked the F1/F2 source paths at the unchanged HEAD. Add a navigation
index and current checkpoint to PROJECT_MEMORY.md; preserve older entries as
historical evidence. Rollback is removal of only the new checkpoint/index text
and document 15. No application fix, migration, deployment or commit is included.

## Continuation 2026-09-08 (second pass)

Add document 19 and its index links in this record and report 15. It verifies
prior claims against primary sources and reverses three of them: the Shortcuts
notification trigger is iOS 27 (not 26); the AccessoryNotifications route is
closed by DPLA 3.3.7(J) rather than merely EU-gated; and ANCS consumption from a
general-purpose computer is reproduced against iOS 26.5 and an iOS 27 beta, so
the blanket "not a Mac and not a PC" claim in report 00 is wrong for Linux and
unproven rather than settled for Windows. Report 00's compliance-by-construction
claim is withdrawn.

Documentation only. No application source, database, dependency, credential,
signing material, deployed service or vendor account was touched, and no Apple
hardware test was performed. Rollback is the removal of document 19 and the two
index entries added for it; leave every other file as found.
