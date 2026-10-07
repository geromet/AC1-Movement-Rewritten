# AC1 Movement Rewritten — Independent Review Evidence

- Reviewed upstream repository: `Banned445/AC1-Movement-Rewritten`
- Reviewed commit: `08703932f713c5b0577ac1e9ca3e872a345c130d`
- Review date: 2026-10-07

## What this branch is

This branch (`review/evidence-08703932`) is rooted at the exact reviewed
commit above. It contains an independent review of the project's movement,
animation and asset subsystems together with reproducer descriptions and an
implementation backlog.

The review was performed independently and every confirmed finding was
source-verified against the reviewed commit. Where a claim was not
runtime-tested, that is stated explicitly in the finding. This branch does not
claim that any fix has been implemented or that the findings have been
runtime-reproduced on the target.

## Scope and status

- The branch contains **independent review/reproducer evidence**. The
  reproducers are described as synthetic fixtures that require no game
  installation.
- Review evidence is **separate from unresolved native-fidelity questions**.
  Items for which original-game evidence is still missing are recorded as
  native-fidelity gaps, not as confirmed defects.
- All findings are stated against the exact reviewed commit above. Upstream
  `main` may have advanced since; fix branches should be based on the
  appropriate current upstream base.

## Contents

- [`CONSOLIDATED-REVIEW.md`](./CONSOLIDATED-REVIEW.md) — the sanitized
  consolidated review, covering confirmed findings, native-fidelity gaps,
  validation gaps and checked non-findings.
- [`IMPLEMENTATION-BACKLOG.md`](./IMPLEMENTATION-BACKLOG.md) — the ten
  implementation packages derived from the review, with finding mappings,
  dependencies and rationale.
- [`validation/`](./validation/README.md) — independent verification status.

## No proprietary assets

No proprietary AC1 assets are redistributed in this repository or on this
branch. All referenced tests and fixtures are synthetic and must not require a
game installation.

## Future fix / PR convention

- Fix branches live in this fork and are used as upstream PR heads.
- Fixes branch from the appropriate current upstream base, not from the review
  evidence branch.
- Review/reproducer evidence stays on `review/evidence-08703932`.
- Upstream PR bodies link to the public evidence paths in this branch.
- PRs must not depend on any private orchestration repository, private branch,
  or private artifact.

## Limitations

- The review executed no `cargo` build/test command and no runtime capture. See
  the execution notes in the consolidated review.
- Synthetic correctness regressions are not native-parity proof. Original-game
  comparison evidence, where used, is reported separately.
