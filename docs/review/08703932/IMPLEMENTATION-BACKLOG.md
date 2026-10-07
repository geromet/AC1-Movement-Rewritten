# AC1 Movement Rewritten — Implementation Backlog

This index turns the consolidated review into ten self-contained implementation
packages. It is derived from the public
[`CONSOLIDATED-REVIEW.md`](./CONSOLIDATED-REVIEW.md) only; it contains no private
orchestration metadata.

All findings are stated against the reviewed commit
`08703932f713c5b0577ac1e9ca3e872a345c130d`.

## Finding accounting

All confirmed findings are assigned exactly once across the ten packages below:

| Package | Findings |
| --- | --- |
| 1 | AC1-REV-002, AC1-REV-003, AC1-REV-009, AC1-REV-011 |
| 2 | AC1-REV-010 |
| 3 | AC1-REV-001, AC1-REV-004, AC1-REV-005, AC1-REV-006, AC1-REV-007 |
| 4 | AC1-REV-008 |
| 5 | AC1-REV-012 |
| 6 | AC1-REV-013 |
| 7 | AC1-REV-016, AC1-REV-018 |
| 8 | AC1-REV-014 |
| 9 | AC1-REV-015, AC1-REV-017 |
| 10 | Validation/CI ladder (cross-cutting) |

Native-fidelity questions and deliberate PORT approximations are intentionally
not included as correctness packages; they remain in the
[native-fidelity gaps](./CONSOLIDATED-REVIEW.md#native-fidelity-gaps) section.

## Implementation order

Dependency/root-cause order, not severity order:

1. **Package 1** — continuous contact/query layer (foundation for the spatial
   packages).
2. **Package 2** — proxy retained-plane solver (independent of package 1).
3. **Package 3** — event-owned/atomic locomotion transitions.
4. **Package 4** — frame-partition-invariant time semantics; coordinates with
   package 3.
5. **Package 5** — rolling animation blend state; coordinates with package 4 if
   animation time semantics change first.
6. **Package 6** — player/map discontinuity reset contract (independent).
7. **Package 7** — installation-scoped runtime configuration/cache; useful
   before expanding real-install validation.
8. **Package 8** — bounded/checked asset decoding.
9. **Package 9** — semantic validation and transactional native-map assembly;
   builds on package 8.
10. **Package 10** — enforceable/truthful validation ladder; can begin
    immediately, some suites depend on packages 1-9.

---

## Package 1 — Replace approximate movement clearance with continuous contact-returning queries

**Findings:** [AC1-REV-002](./CONSOLIDATED-REVIEW.md#bounded-conservative-advancement-can-cross-collision-geometry-and-return-no-hit),
[AC1-REV-003](./CONSOLIDATED-REVIEW.md#wall-run-entry-fabricates-cardinal-normals-on-authored-angled-geometry),
[AC1-REV-009](./CONSOLIDATED-REVIEW.md#sphere_free_distance-is-a-discrete-2-cm-sampler-that-can-miss-thin-obstacles),
[AC1-REV-011](./CONSOLIDATED-REVIEW.md#guidance-driven-traversal-can-interpolate-through-solid-blockers-because-native-corridor-sweeps-are-omitted).

**Problem summary.** Several spatial defects share one root cause: callers
consume approximate scalar clearance instead of a continuous contact result.
The bounded conservative-advancement cast can exhaust its iteration budget after
the requested motion has crossed geometry and then report no hit; wall-run entry
reconstructs a cardinal normal instead of using the authored surface normal; the
discrete sphere-free-distance sampler can miss thin obstacles between samples;
and guidance-driven traversal interpolates a path whose corridor was never
swept.

**Rationale.** These four defects are grouped because they should converge on a
single continuous, contact-returning query API rather than four symptom-specific
probes. The API must return at least contact fraction/distance, contact point,
authored normal, collider identity, and the query/filter/layer provenance
actually used. Fixing these first removes the spatial foundation other packages
may otherwise duplicate.

**Dependencies.** None. Should land before local fixes that would duplicate
sweep logic. Resolving the native layer/filter risk requires original-game
evidence and is out of scope here.

---

## Package 2 — Make the proxy four-plane fallback satisfy every retained constraint

**Finding:** [AC1-REV-010](./CONSOLIDATED-REVIEW.md#four-plane-solver-fallback-can-return-a-velocity-violating-a-retained-constraint).

**Problem summary.** The four-active-plane fallback chooses a plane to discard
independently of the triple it solved, so the returned velocity may not satisfy
the retained constraints. Severe penetration/tight-concavity recovery can then
move into a surface, jitter, or depend on insertion order.

**Rationale.** The solver invariant is defective regardless of how often the
synthetic cavity occurs in shipped geometry. It is kept separate from the
continuous-query work because the change is local to active-plane
reduction/solution and can proceed independently.

**Dependencies.** None; independent of package 1.

---

## Package 3 — Make locomotion transitions event-owned and apply destination state atomically

**Findings:** [AC1-REV-001](./CONSOLIDATED-REVIEW.md#historical-landing-state-is-replayed-as-a-fresh-landing-on-later-ground-activation),
[AC1-REV-004](./CONSOLIDATED-REVIEW.md#destination-activation-latency-depends-on-context-system-position),
[AC1-REV-005](./CONSOLIDATED-REVIEW.md#interrupted-climb-to-ground-continuation-leaks-into-a-later-climb-lifetime),
[AC1-REV-006](./CONSOLIDATED-REVIEW.md#inairentryfall-carries-velocity-but-the-destination-ignores-it),
[AC1-REV-007](./CONSOLIDATED-REVIEW.md#air-catch-classification-survives-unrelated-inair-entries).

**Problem summary.** Historical or context-local state is reused as pending
transition state, and destination activation and `Body` handoff are spread
across ordered systems. Durable landing history is replayed as a fresh landing;
destination activation timing depends on chained system position; interrupted
Climb completion state leaks into a later Climb; `Fall { velocity }` is ignored
so stale body velocity survives; and catch classification persists across
unrelated entries.

**Rationale.** All five are the same lifecycle bug, so one transition contract
(separating history from pending events, atomic destination/`Body` handoff, an
activation epoch, fresh-lifetime resets, and event-owned reception
classification) should address them together. A full state-machine rewrite is
not required if the invariants can be met with a smaller transaction/generation
layer.

**Dependencies.** None, though the exact timing policy should be chosen once for
the whole context graph; coordinates with package 4.

---

## Package 4 — Define frame-partition-invariant time semantics for movement, input, animation and IK

**Finding:** [AC1-REV-008](./CONSOLIDATED-REVIEW.md#elapsed-time-handling-is-not-frame-partition-invariant-across-movement-input-animation-and-ik).

**Problem summary.** Movement, input, animation, IK and MoveBlend each consume
elapsed time differently (independent caps, discarded boundary overshoot,
partial cycle consumption). Equivalent elapsed intervals can therefore produce
different gameplay/visual state depending on how the interval was partitioned.

**Rationale.** This is a cross-system correctness/invariance issue that cannot
be fixed by giving one subsystem another private clamp. It requires one declared
runtime time/substep contract that consumes the whole interval.

**Dependencies.** Coordinate with package 3 because destination activation
epochs are part of actor-update timing semantics.

---

## Package 5 — Keep rolling animation pose, root and playback state coherent across interrupted blends

**Finding:** [AC1-REV-012](./CONSOLIDATED-REVIEW.md#rolling-a-animation-blends-advance-pose-and-root-from-incompatible-a-states).

**Problem summary.** On interrupted rolling-A blends, the outgoing pose can be
advanced/re-sampled while the outgoing root remains frozen or is evaluated from
a different state, producing a one-frame pose discontinuity and foot/body slide.

**Rationale.** Pose, root and playback rate must be represented as one coherent
evaluatable outgoing state; this is a focused animation-state fix rather than a
redesign of animation selection.

**Dependencies.** Coordinate with package 4 if animation elapsed-time semantics
are changed first.

---

## Package 6 — Establish one player/map discontinuity reset contract

**Finding:** [AC1-REV-013](./CONSOLIDATED-REVIEW.md#map-reset-leaves-old-world-limbik-state-attached-to-the-player).

**Problem summary.** Map/player reset reconstructs core gameplay/animation state
but leaves stateful visual data, concretely old-world world-space `LimbIk`
goals/weights, attached to the player into the first frame in the new world.

**Rationale.** The defect is the absence of a single reset/discontinuity
transaction spanning gameplay and rendering state. The fix centralizes ownership
of the reset epoch so new stateful subsystems cannot silently bypass reset.

**Dependencies.** None.

---

## Package 7 — Scope runtime game-data configuration and caches to an installation/App lifetime

**Findings:** [AC1-REV-016](./CONSOLIDATED-REVIEW.md#process-global-game-dataconfig-caches-are-not-scoped-to-an-installation-or-app-lifetime),
[AC1-REV-018](./CONSOLIDATED-REVIEW.md#persisted-game-directory-configuration-is-relative-path-fragile-and-platform-incomplete).

**Problem summary.** The game directory and shader-resource caches are
process-global and sticky, not keyed to an installation; persisted game
directory paths are not canonicalized; and environment override behavior can
suppress otherwise valid discovery.

**Rationale.** Both findings are the same "configuration/cache lifetime"
problem and should be fixed together as a typed, installation/App-scoped runtime
configuration with canonical paths and per-install cache keys.

**Dependencies.** Helpful before expanding real-install validation in package
10.

---

## Package 8 — Add bounded checked failure boundaries to Forge, skeleton and animation decoding

**Finding:** [AC1-REV-014](./CONSOLIDATED-REVIEW.md#external-asset-decoding-lacks-a-uniform-boundedchecked-failure-boundary).

**Problem summary.** Local game-data decoding does not uniformly prove counts,
ranges, graph structure and allocation sizes before trusting them, so malformed
or version-different local data can cause cycles, count bombs, arithmetic
failures, panics, oversized allocation, or silent partial success.

**Rationale.** This is robustness work for malformed local data (no
remote-security claim). It establishes the checked-reader/error-taxonomy
infrastructure that package 9 builds on.

**Dependencies.** None.

---

## Package 9 — Add semantic asset validation and transactional native-map part assembly

**Findings:** [AC1-REV-015](./CONSOLIDATED-REVIEW.md#structurally-plausible-asset-data-can-be-accepted-with-ambiguous-or-invalid-semantics),
[AC1-REV-017](./CONSOLIDATED-REVIEW.md#optional-village-placement-failure-is-not-transactionally-rolled-back).

**Problem summary.** Byte-complete data can pass syntactic parsing while
violating semantic invariants (unresolved/duplicate ids, invalid
enum/cardinality, non-finite values, ambiguous resource matches, unsupported
transforms), and an optional native-map part can partially mutate shared
collections before a later failure.

**Rationale.** These address a different boundary than package 8: an input can
be in-bounds and still be semantically invalid or ambiguous, and assembly should
be all-or-nothing. Grouping them yields one semantic-validation plus
transactional-commit change.

**Dependencies.** Package 8 establishes common decode/error infrastructure.

---

## Package 10 — Make the validation ladder enforceable and distinguish zero-data from AC1-install certification

**Findings:** Validation/CI gaps (cross-cutting; supports every confirmed
finding).

**Problem summary.** The repository has a large test corpus but no
machine-enforced validation boundary, tests that can return early when game data
is absent, a hand-maintained harness that does not represent the production
schedule, and no required frame-rate/hitch matrix. A green zero-data run can
therefore be mistaken for asset-integration/native-parity certification.

**Rationale.** A truthful validation ladder is the enforcement envelope for the
other nine packages. Each job must state exactly what it certifies: a green
public zero-data job means synthetic/source invariants passed, not that AC1
asset integration was validated. No proprietary data enters public CI.

**Dependencies.** The zero-data gate can begin immediately; individual
regression suites can be added as packages 1-9 land.

---

## Future fix / PR convention

- Fix branches are created in this fork from the appropriate current upstream
  base and used as upstream PR heads.
- Review/reproducer evidence stays on `review/evidence-08703932`.
- Upstream PR bodies link to the public evidence paths in this branch.
- PRs must not depend on any private orchestration repository, private branch,
  or private artifact.
