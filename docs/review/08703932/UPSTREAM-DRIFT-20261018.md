# Upstream drift preflight for review publication — 2026-10-18

This supersedes the 2026-10-15 [historical drift preflight](./UPSTREAM-DRIFT-20261015.md) with a newly checked upstream HEAD. This is a **currentness warning**, not a re-review, runtime verification, or finding-resolution report. The independent findings and [ten-package backlog](./IMPLEMENTATION-BACKLOG.md) are historical evidence tied to one source revision. Do not publish them as newly confirmed defects on a later head without rechecking the affected code and reproducing the tests.

## Exact public repository state observed

| Surface | Commit / result |
| --- | --- |
| Original reviewed upstream `main` | [`08703932f713c5b0577ac1e9ca3e872a345c130d`](https://github.com/Banned445/AC1-Movement-Rewritten/commit/08703932f713c5b0577ac1e9ca3e872a345c130d) |
| Upstream `main` at this preflight | [`f53ac663f995914833b33ead5c38ad43a5b3260c`](https://github.com/Banned445/AC1-Movement-Rewritten/commit/f53ac663f995914833b33ead5c38ad43a5b3260c) |
| Fork `main` at this preflight | [`28a4cbafa8af7873a8331c35d7a17d936c1dc48d`](https://github.com/geromet/AC1-Movement-Rewritten/commit/28a4cbafa8af7873a8331c35d7a17d936c1dc48d) |
| Public historical evidence branch before this note | [`879a215f355a06f3e1eac3479d90f87d3524cebb`](https://github.com/geromet/AC1-Movement-Rewritten/commit/879a215f355a06f3e1eac3479d90f87d3524cebb) |

The [upstream comparison](https://github.com/Banned445/AC1-Movement-Rewritten/compare/08703932f713c5b0577ac1e9ca3e872a345c130d...f53ac663f995914833b33ead5c38ad43a5b3260c) reported **396 commits ahead, 0 behind, 92 changed paths**. Many paths central to the review have changed. This comparison does **not** prove that a finding is fixed, still present, or newly introduced. The fork `main` is separately behind the observed upstream head; do not use the fork default as a substitute for an upstream currentness check.

## Package impact triage

Every path listed below appeared as changed in the commit-pinned upstream comparison. The right column is a **required verification direction**, not a completed test.

| Package | Changed paths relevant to the original finding | Next verification |
| --- | --- | --- |
| 1 — Continuous contact queries | `src/proxy.rs`, `src/collision.rs`, `src/player/walling.rs`, `src/guidance.rs`, `src/player/ledge.rs`, `src/player/ladder.rs` | Re-run shallow-incidence, authored-normal, thin-blocker and traversal-corridor cases |
| 2 — Four-plane fallback | `src/proxy.rs` | Inspect retained-plane solution against current solver and run overconstrained cavity permutations |
| 3 — Atomic locomotion transitions | `src/player/ground.rs`, `src/player/climb.rs`, `src/player/mod.rs`, `src/player/air.rs`, `src/player/ground_tree.rs` | Audit event ownership and transition epochs after current state-machine changes |
| 4 — Frame-partition time semantics | `src/main.rs`, `src/input.rs`, `src/anim.rs`, `src/ik.rs` | Compare one large elapsed step against equivalent subdivisions |
| 5 — Rolling animation | `src/anim.rs`, `src/anim_pose_tests.rs`, `src/player/move_blend.rs` | Re-run interrupted rolling-blend pose/root continuity cases |
| 6 — Player/map reset | `src/map_menu.rs`, `src/ik.rs`, `src/player/mod.rs` | Re-run map-switch, despawn/respawn, IK and optional-component reset cases |
| 7 — Installation-scoped resources | `src/assets/mod.rs`, `src/assets/altair.rs`, `src/map_menu.rs` | Test two installation contexts and repair/reload behavior in one process |
| 8 — Bounded asset decoding | `src/assets/forge.rs`, `src/assets/anims.rs`, `src/assets/tests.rs` | Re-run synthetic malformed-count, overflow, range and allocation-limit cases |
| 9 — Transactional native-map assembly | `src/native_map.rs`, `src/assets/world.rs`, `src/assets/forge.rs` | Check semantic validation and rollback on failed map-part assembly |
| 10 — Validation/CI ladder | `src/sim_tests.rs`, `src/anim_pose_tests.rs; no .github/ path in this comparison` | Inspect current workflows and test gates separately; do not infer CI coverage from the diff alone |

The rows only enumerate representative changed paths relevant to each package, not every one of the 92 paths, and do not establish whether the changes altered the specific defective behavior.

## Publication gate

1. Re-read the current upstream implementation at the *exact head selected for publication*. Reproduce each candidate finding (or document a grounded supersession) and capture precise code locations.
2. Verify package-to-package API dependencies and run synthetic regression tests at that same revision. Record exact commands, exit status, and skipped cases; distinguish source inspection from test execution.
3. Keep native/game-installation fidelity questions separate from port correctness findings. Do not imply AC1 native parity from zero-data fixtures.
4. Check public upstream issue/PR duplicates again before publishing each issue; a historical empty issue list is not a current duplicate check.
5. Open self-contained issue(s) only through an explicitly authorized upstream write identity or have an upstream maintainer publish them. A writable personal fork does not grant issue-write access to the upstream repository.
6. Keep fix branches and independently verified evidence separate; no default-branch merge or upstream mutation is warranted by this document.

**Verification outcome:** public commit comparison and changed-file enumeration completed. No current-head finding-by-finding revalidation, `cargo check`, `cargo test`, CI execution, original-game experiment, or upstream issue publication was performed for this preflight. The historical ten-package synthesis remains useful as a *starting hypothesis*, not a current-head acceptance result.
