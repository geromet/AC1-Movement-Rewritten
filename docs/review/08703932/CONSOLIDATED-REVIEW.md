# AC1 Movement Rewritten — Consolidated Review

Independent review evidence. This document is a sanitized public copy of a
source-verified review of the movement, animation and asset subsystems.

## Provenance

- Target repository: `Banned445/AC1-Movement-Rewritten`
- Target commit: `08703932f713c5b0577ac1e9ca3e872a345c130d`
- Review date: 2026-10-07
- Review nature: independent, multi-pass source review. Every confirmed
  finding was source-verified at the exact target commit above. Where a claim
  was or was not runtime-tested, that is stated per finding.
- Target repository mutation: none. The target repository was read only.

### How the review was executed

No `cargo build`, `cargo check`, `cargo test`, `cargo fmt`, `cargo clippy`,
fuzz target, or runtime capture was executed by the review.

- Movement/state inspection: commit-pinned source inspection; complete
  `src/sim_tests.rs` inspection; static consistency checks including
  `ITEM_FLAGS` ordering and duplicate jump-clip identity.
- Spatial inspection: commit-pinned source/API inspection and source-test
  inspection. Native-map tests requiring a local installation were not
  executed.
- Animation/visual inspection: commit-pinned source inspection of
  animation, IK, cloth and visual systems plus scheduling, reset and
  model-loading and test/capture infrastructure. No runtime captures were
  executed.
- Asset-boundary inspection: commit-pinned parser/import source inspection.
  No build, `cargo test`, fuzzing, or proprietary-file-dependent test
  execution.
- Architecture/reliability inspection: repository, tree, source, status,
  workflow, ruleset, open-issue and open-PR inspection. No local Rust command
  was executed.

Therefore the evidence categories are:

- Tests actually executed and passed: none.
- Tests actually executed and failed: none.
- Proprietary-data tests executed: none.
- Tests requiring proprietary AC1 data: inspected in source only; not
  executed. Several are coded to return early when data is absent.
- Reviewer-proposed regression/fuzz/capture tests: not executed unless
  explicitly described below as a review-side arithmetic reproduction.
- Manual/original-game comparisons: not executed.

### Review-pass commands and checks

The review independently:

1. resolved the target repository and its refs through the public GitHub API;
2. fetched target files directly at the immutable reviewed commit;
3. independently inspected the code mechanisms for every High finding and
   several disputed Medium findings;
4. reproduced the shallow-incidence conservative-advancement fixture
   arithmetically (see the relevant finding);
5. attempted a local pinned checkout with `git clone` / `git checkout
   --detach` at the reviewed commit. The checkout failed before cloning
   because the local container could not resolve github.com, so no cargo
   command was executed.

Source verification continued through commit-addressed reads. A failed local
clone is not counted as a failed target test.

## Executive assessment

The project has a substantial deterministic movement-test corpus and generally
careful reverse-engineering annotations, but the reviewed baseline still has
three serious normal-operation correctness failures: landing history can be
replayed as a fresh fatal landing, the fundamental capsule sweep can
false-clear after bounded conservative-advancement exhaustion, and wall-run
entry reconstructs a cardinal normal instead of consuming the authored
collision normal.

Below those are several narrower state-lifetime, frame-partition,
collision-query, animation, reset, and asset-boundary defects. The most
important systemic pattern is ownership ambiguity: historical state is reused
as transition state, different subsystems advance different notions of time,
and several spatial/visual transitions approximate a richer native query or
lifecycle boundary without one authoritative transaction.

The review also contains important fidelity questions and validation gaps.
Those are not inflated into High correctness bugs merely because the native
behavior is unresolved or CI is absent.

## Confirmed findings

No Critical findings.

### [AC1-REV-001] Historical landing state is replayed as a fresh landing on later Ground activation

- Severity: High
- Confidence: High
- Components: src/player/ground.rs; Ground transition lifecycle
- Exact code evidence: HumanGroundData.last_landing is durable state at ground.rs:81. HumanGroundData::enter at 177-201 resets speed/oneshot on landing == None but does not clear or consume last_landing. update_ground at 236-249 checks last_landing whenever loco.just_switched is true, re-applying camera shake at 241-242 and fatal respawn/body velocity reset at 244-247.
- Failure mechanism: last_landing is simultaneously treated as history and as a pending one-shot entry event. Non-landing Ground entries therefore replay the previous landing side effects.
- Trigger/reproducer: perform a fatal fall, allow the first Ground entry to respawn, enter Climb/Ledge/Ladder/PassOver without another air landing, then return to Ground with landing: None. The first Ground activation processes the stale Fatal value again.
- User-visible consequence: an unrelated traversal exit can teleport the player back to spawn; a previous >3 m nonfatal landing can restart camera shake.
- Recommended repair: separate durable last_landing history from a consumable pending_landing_effect, or apply landing side effects at the landing transition and keep last_landing observational only.
- Regression test: fatal fall -> first respawn -> non-air Ground round trip -> first Ground update; assert feet/velocity/shake are not replayed while historical last_landing may remain available.
- Requires proprietary AC1 data: no
- Verification status: independently source-verified at the exact target SHA; not runtime-executed.

### [AC1-REV-002] Bounded conservative advancement can cross collision geometry and return no hit

- Severity: High
- Confidence: High
- Components: src/proxy.rs::linear_cast; character capsule controller
- Exact code evidence: linear_cast at proxy.rs:160-196 uses displacement length as its advancement denominator. It allows only 40 iterations at 177, marks a hit only when distance <= 1e-4 at 178-180, advances t by c.dist / len at 182, and emits a Contact only when hit became true at 188-192. Iteration exhaustion is therefore indistinguishable from clear path.
- Failure mechanism: at shallow incidence, total displacement can be much larger than normal closing speed. Each step is individually conservative, but convergence can require more than 40 iterations. If the cap is reached while the actual displacement crosses the surface, the function returns no collision.
- Trigger/reproducer: capsule-surface gap 0.101 m; displacement 0.102 m into the surface plus 0.75 m tangentially. True contact occurs at fraction about 0.990196. The implemented 40-step recurrence remains at t about 0.987168 with residual separation about 0.0003089 m, so hit remains false even though the requested motion crosses the surface.
- User-visible consequence: under a sufficiently large/shallow movement step, the controller can end on the wrong side of a wall/triangle; a later closest-point normal can then preserve that wrong side.
- Recommended repair: use closing speed along the separating normal with explicit progress/convergence handling, or a robust swept capsule/triangle/AABB TOI implementation. Never interpret non-convergence as clear path; invoke a fallback or conservatively reject the move.
- Regression test: synthetic vertical wall with the numbers above; compare one large movement step with subdivided steps and assert both remain on the original side.
- Requires proprietary AC1 data: no
- Verification status: independently source-verified and independently arithmetic-reproduced at review time; target Rust test not executed.

### [AC1-REV-003] Wall-run entry fabricates cardinal normals on authored angled geometry

- Severity: High
- Confidence: High
- Components: src/player/walling.rs::face_normal; wall_ahead; collision queries
- Exact code evidence: walling.rs:188 explicitly describes face_normal as an axis-aligned greybox face approximation while the game uses the collision hit normal. face_normal at 189-202 tests only +X, -X, +Z, -Z and otherwise defaults to -toward. wall_ahead at 208-228 obtains only scalar distance from sphere_free_distance, then calls face_normal at 216; the reconstructed normal drives the 45-degree acceptance test at 217-218, side frame at 220, support probes at 221-223, and final warp contact at 227.
- Failure mechanism: imported rotated boxes and mesh triangles can have non-cardinal normals. The scalar query discards that surface orientation and the caller invents one. The invented normal can change both the admission decision and the movement frame.
- Trigger/reproducer: an authored wall whose true outward normal is around (-0.5, 0, 0.866) while the player probes along +X. A true 45-degree-facing test rejects because the opposing component is only 0.5, but a reconstructed -X gives dot 1.0 and accepts.
- User-visible consequence: walling can start on a surface whose actual angle should reject it, choose the wrong left/right tangent, probe width in the wrong frame, and warp the root to a physically incorrect contact.
- Recommended repair: make the spatial query return a contact record containing fraction/distance, point, exact contact normal, collider identity and filter/layer provenance. Consume that contact directly in walling.
- Regression test: rotated thin-box/triangle walls at 30, 60 and 80 degrees; assert entry decision and returned normal against authored geometry.
- Requires proprietary AC1 data: no
- Verification status: independently source-verified at the exact target SHA; not runtime-executed.

### [AC1-REV-004] Destination activation latency depends on context-system position

- Severity: Medium
- Confidence: High
- Components: src/player/mod.rs; all context update systems
- Exact code evidence: switch_context at player/mod.rs:158-203 immediately changes loco.current and sets loco.just_switched = true. PlayerPlugin at 228-237 chains Ground -> Air -> Ledge -> Climb -> Hay -> Walling -> Narrow -> Ladder in one Update schedule.
- Failure mechanism: a forward transition such as Ground -> Air reaches the destination later in the same chain, which consumes just_switched immediately; a backward transition such as Air -> Ground occurs after Ground already ran, so just_switched survives into the next frame. Normal destination logic therefore begins at different actor-update boundaries solely because of chain order.
- Trigger/reproducer: exact-frame comparison of Ground -> Air versus Air -> Ground, and Ledge -> Climb versus Climb -> Ledge.
- User-visible consequence: one-update differences in input windows, root integration and action timing; at capped 20 Hz movement time this can represent 50 ms.
- Recommended repair: use a per-actor update epoch/transition generation or one active-context dispatcher so destination activation semantics do not depend on source-code scheduling order.
- Regression test: transition matrix asserting identical actor-update boundary semantics for forward/backward cyclic edges.
- Requires proprietary AC1 data: no
- Verification status: source-verified at target SHA; not runtime-executed.

### [AC1-REV-005] Interrupted climb-to-ground continuation leaks into a later Climb lifetime

- Severity: Medium
- Confidence: High
- Components: src/player/climb.rs
- Exact code evidence: HumanClimbData.to_ground is context-local completion state at climb.rs:319. Fresh entry flows through enter/set_entry_holds around 354-409 without resetting it. It is set on climb-to-ground paths at 1229 and 1316 and only normally consumed/reset in move completion at 1084-1093. The lost-grip path can leave Climb before that consumption.
- Failure mechanism: if grip is lost while a to-ground move is active, the context exits with to_ground still true. A later unrelated Climb entry reuses the same HumanClimbData and can complete its new move into the stale branch.
- Trigger/reproducer: begin a move that sets to_ground, remove supporting guidance before completion, fall to InAir, restore/re-enter Climb, then let the new entry interpolation complete.
- User-visible consequence: an unrelated later climb can unexpectedly eject to Ground.
- Recommended repair: clear to_ground and other completion-only fields on every fresh Climb entry/reset block; represent intentionally persistent continuation state explicitly in ClimbEntry.
- Regression test: interrupted to-ground move -> lost grip -> fresh Climb -> completion must remain in Climb.
- Requires proprietary AC1 data: no
- Verification status: source-verified from review evidence and target source; not runtime-executed.

### [AC1-REV-006] InAirEntry::Fall carries velocity but the destination ignores it

- Severity: Medium
- Confidence: High
- Components: src/player/air.rs; src/player/climb.rs; src/player/mod.rs
- Exact code evidence: InAirEntry::Fall declares velocity at air.rs:118. HumanInAirData::enter matches Fall at 297-305 as { from, origin, speed_param, .. }, discarding velocity. switch_context only receives Locomotion and HumanDataBundle, not Body, so the transition layer has no other place to apply the requested body velocity.
- Failure mechanism: callers can explicitly request a new Fall velocity, including Vec3::ZERO, but stale Body.velocity from the previous context survives. The fixed context scheduling can then coast that stale velocity before the destination overwrites it.
- Trigger/reproducer: enter Climb with nonzero horizontal Body.velocity, invalidate holds before Climb reaches its ordinary velocity-zeroing path, request lost-grip Fall with velocity zero, then advance through Air activation.
- User-visible consequence: a lost-grip fall can receive a one-frame horizontal shove that the transition explicitly attempted to discard.
- Recommended repair: make transition application own Body handoff atomically, or at minimum apply Fall.velocity before switching at every caller.
- Regression test: nonzero previous velocity + Fall { velocity: ZERO } must produce zero horizontal displacement/velocity across the activation frame.
- Requires proprietary AC1 data: no
- Verification status: independently source-verified for the ignored field; concrete runtime sequence not executed.

### [AC1-REV-007] Air catch classification survives unrelated InAir entries

- Severity: Medium
- Confidence: High
- Components: src/player/air.rs; src/player/ledge.rs; src/anim.rs
- Exact code evidence: HumanInAirData.long_catch is at air.rs:158. enter at 190-200 resets entry-local target/timing/fall fields but not long_catch. The field is assigned only by free-fall catch classification at 637. Ledge animation selection later reads prior Air long_catch for the grab variant.
- Failure mechanism: a long free-fall catch sets long_catch=true. A later target-driven InAir entry does not refresh it, and the subsequent ledge reception can select the old catch class.
- Trigger/reproducer: >3 m free-fall ledge catch -> leave ledge -> ordinary short target jump to another ledge.
- User-visible consequence: the later ordinary reception can play the long-fall grab/catch animation.
- Recommended repair: reset long_catch on every Air entry and explicitly set it for the current event; better, carry reception classification in the receiving LedgeEntry instead of ambient previous-context storage.
- Regression test: long catch followed by short target catch must select the second event's short classification.
- Requires proprietary AC1 data: no
- Verification status: source-verified at target SHA; animation output not runtime-captured.

### [AC1-REV-008] Elapsed-time handling is not frame-partition invariant across movement, input, animation and IK

- Severity: Medium
- Confidence: High
- Components: context updates; src/input.rs; src/anim.rs; src/ik.rs; src/player/move_blend.rs
- Exact code evidence:
  - movement context updates cap dt with time.delta_secs().min(1.0 / 20.0), e.g. ground.rs:230;
  - input ages legs_pressed_ago by full time.delta_secs() at input.rs:111;
  - IK separately caps dt at ik.rs:358;
  - anim.rs advances to the next multi-item action at a boundary by setting p.phase = 0.0 around 1126, discarding overshoot, while fade receives the whole frame dt at 1138;
  - MoveBlend advances phase by dt / t at move_blend.rs:228 but consumes only one crossed cycle.
- Failure mechanism: the same real elapsed interval is represented differently by interacting subsystems. Long frames can age input faster than movement/action time, leave IK behind gameplay/animation, lose animation-item residual time, or leave normalized gait phase outside its intended cycle.
- Trigger/reproducer: compare identical locked-action/buffered-input and gait/IK sequences at 60, 20, 10 Hz and with 100-250 ms hitches; compare one long elapsed update with equivalent bounded substeps.
- User-visible consequence: buffered input acceptance, visual contact release, gait phase and action completion can change with frame partition rather than gameplay state.
- Recommended repair: establish one locomotion/visual simulation time domain and consume the full elapsed interval through bounded substeps where numerical stability requires caps. Preserve boundary residuals rather than discarding time.
- Regression test: frame-partition invariance suite across input buffer, multi-item animation, IK and MoveBlend.
- Requires proprietary AC1 data: no
- Verification status: independently source-verified for the conflicting clocks/boundary logic; no full runtime timing matrix executed.

### [AC1-REV-009] sphere_free_distance is a discrete 2 cm sampler that can miss thin obstacles

- Severity: Medium
- Confidence: High
- Components: src/collision.rs::sphere_free_distance; walling/ledge query callers
- Exact code evidence: collision.rs:186-202 uses step = 0.02 and only tests sphere centers at origin + dir * (d + step). It never computes continuous time of impact between samples.
- Failure mechanism: with query radius 0.01 m, an obstacle between samples can intersect the continuous swept sphere while being farther than r from both sampled centers.
- Trigger/reproducer: tiny vertical triangle at x=0.01 m, about 0.005 m off centerline, sweep along +X with radius 0.01 m. Continuous distance is inside radius, while the x=0.02 sampled center can be outside it.
- User-visible consequence: a wall/support/clearance predicate can falsely report free space, changing wall-run, ledge or shimmy behavior.
- Recommended repair: use continuous swept sphere-vs-AABB/triangle TOI and share it with the broader continuous-query layer.
- Regression test: mid-sample obstacle plus tangent/inside/outside-radius cases.
- Requires proprietary AC1 data: no
- Verification status: source-verified at target SHA; synthetic Rust fixture not executed.

### [AC1-REV-010] Four-plane solver fallback can return a velocity violating a retained constraint

- Severity: Medium
- Confidence: High for algorithmic defect; Medium for ordinary-game reachability
- Components: src/proxy.rs::solve_active
- Exact code evidence: proxy.rs:430-451 handles four active planes. If reduction fails, it chooses worst at 444, computes v from active[0], active[1], active[3] at 445, then independently swap_removes active[worst] at 446 and returns the precomputed v at 450.
- Failure mechanism: unless worst happens to be the plane omitted from the solved triple, the returned velocity was not solved against the exact three constraints that remain active.
- Trigger/reproducer: synthetic four-plane overconstrained cavity such as +X, +Y, +Z and (-1,-1,-1)/sqrt(3), with equal recovery requirements; permute insertion order.
- User-visible consequence: severe penetration/tight concavity recovery can move farther into a surface, jitter, or become insertion-order dependent.
- Recommended repair: choose/remove the discarded plane first, then solve exactly the retained set; assert the returned velocity satisfies every retained plane within epsilon.
- Regression test: four-plane tetrahedral cavity plus insertion-order permutations.
- Requires proprietary AC1 data: no
- Verification status: independently source-verified; reachability in ordinary shipped geometry not established.

### [AC1-REV-011] Guidance-driven traversal can interpolate through solid blockers because native corridor sweeps are omitted

- Severity: Medium
- Confidence: High
- Components: src/player/ledge.rs; src/player/ledge_moves.rs
- Exact code evidence: side_climb_entry documents that native clearance tests are not modelled. try_side_to_ladder documents the native lateral capsule sweep and explicitly states PORT: no clearance sweep. LedgeMove/root update interpolates the root directly, and ledge.rs:476-477 assigns the interpolated position to body.feet without running the character proxy along that path.
- Failure mechanism: endpoint guidance and endpoint fit can both be valid while a thin pillar/wall intersects the corridor between start and destination. The committed animation/root interpolation crosses it because the path itself was never cleared.
- Trigger/reproducer: valid ledge and valid ladder/climb target about 1 m apart with a thin vertical torso blocker halfway between them.
- User-visible consequence: the character can travel through a solid obstacle during a semantically valid traversal transition.
- Recommended repair: port the missing body/corridor clearance before committing the move, preferably through the same continuous layer/filter-aware swept query used by the controller.
- Regression test: ledge-to-ladder and ledge-to-climb fixtures with clear endpoints but blocked midpoint.
- Requires proprietary AC1 data: no
- Verification status: source-verified; documented PORT approximation becomes a correctness defect for the blocker geometry above.

### [AC1-REV-012] Rolling-A animation blends advance pose and root from incompatible A states

- Severity: Medium
- Confidence: High
- Components: src/anim.rs
- Exact code evidence: start_blend at anim.rs:994-1017 captures the currently displayed pose/root into prev/prev_root at 1003-1004 and installs a_roll for blend kinds 1/3 at 1016-1017. During fading, a_roll advances at 1133-1136 and the pose source is re-sampled and overwrites prev at 1166-1173. Root handling at 1191-1197 does not evaluate rolling A root motion: FROMAONLY uses frozen prev_root and BLENDAB interpolates frozen prev_root against current B.
- Failure mechanism: transition interruption can first capture the actual composite pose on screen, then replace that source with a pure re-sampled outgoing item. Simultaneously, pose A advances while root A remains frozen.
- Trigger/reproducer: synthetic A->B blend interrupted around 50% by an AROLLBROLL B->C transition; or rolling A with nonlinear root motion and FROMAONLY.
- User-visible consequence: one-frame pose discontinuity and/or foot/body slide during rolling transitions.
- Recommended repair: represent A as an evaluatable outgoing blend-tree state including pose, root and playback rate. At minimum preserve the captured composite and evaluate current rolling-A displacement consistently.
- Regression test: synthetic one-bone A/B/C continuity test plus rolling-A root-displacement test.
- Requires proprietary AC1 data: no
- Verification status: independently source-verified; visual capture not executed.

### [AC1-REV-013] Map reset leaves old-world LimbIk state attached to the player

- Severity: Medium
- Confidence: High
- Components: src/map_menu.rs; src/ik.rs; player visual lifecycle
- Exact code evidence: reset_player at map_menu.rs:146-168 reinserts player_components and Transform and resets AnimPlayer at 151-153, but does not reset LimbIk. LimbState.goal is explicitly world-space at ik.rs:46; LimbIk is a separate component at 190-205 whose release path fades state instead of atomically invalidating it.
- Failure mechanism: switching maps can teleport/reconstruct gameplay state while old world-space IK goals, weights, pelvis/ground adjustment and cached chain state remain. With paused/zero virtual time, fade-out does not progress until after the switch.
- Trigger/reproducer: seed a player with a weight-1 hand goal in the old map, switch maps/reset player, then run the first animation/IK frame without new limb targets.
- User-visible consequence: transient stretched/snapped limbs or stale pelvis correction from coordinates in the previous world.
- Recommended repair: make reset_player the owner of a single visual/gameplay discontinuity transaction; explicitly reset LimbIk and other stateful visual caches through a reset epoch rather than relying on distance/hitch heuristics.
- Regression test: poison locomotion/proxy/AnimPlayer/LimbIk/visual/cloth/pad/camera state, reset, and assert the defined post-reset state before the first rendered pose.
- Requires proprietary AC1 data: no for the synthetic reset test; real-install map-switch variant should additionally be run.
- Verification status: independently source-verified; runtime map-switch capture not executed.

### [AC1-REV-014] External asset decoding lacks a uniform bounded/checked failure boundary

- Severity: Medium
- Confidence: High
- Components: src/assets/forge.rs; src/assets/ac_formats.rs; src/assets/ac_anim.rs; src/native_map.rs
- Exact code evidence includes:
  - Forge::open loops through block links while block != 0/u64::MAX at forge.rs:66 without a visited-set bound, and converts count to usize at 73;
  - Forge container allocation uses aggregate declared sizes in Vec::with_capacity at 134;
  - resources computes hdr = size - dsz at 171 before establishing that dsz <= size;
  - LZO allocates out_len at 198 and checks exact output length only after decoding at 280-281;
  - parse_skeleton starts with only while i + 9 < d.len() at ac_formats.rs:208 but then unwraps deeper Bone fields at 211-212;
  - ac_anim preallocates untrusted counts with Vec::with_capacity at 117/343/353 and converts any event decode error to an empty event list at 335;
  - native-map GuidanceSystem marker handling has a checked-prefix gap.
- Failure mechanism: malformed, damaged or version-different local installation data can create cycles, count bombs, underflow/out-of-range reads, parser panics, oversized allocation, or silent partial success before structural validation has established the required bounds/invariants.
- Trigger/reproducer: synthetic cyclic Forge index; huge count; truncated resource TOC; LZO output beyond declared length; truncated Bone marker; animation count bombs; GuidanceSystem marker before its required prefix.
- User-visible consequence: local hang, panic, OOM, or misleading successful load of incomplete animation data. No remote-code-execution or sandbox-boundary claim is supported.
- Recommended repair: make every successful decoder establish bounded counts, checked ranges/arithmetic, finite numeric domains and structural invariants before allocation/use; apply decode budgets; distinguish CorruptData from UnsupportedVersion/Layout.
- Regression test: prefix truncation/property tests and fuzz targets for Forge, skeleton and animation decode; all arbitrary inputs must terminate without panic and within an explicit allocation budget.
- Requires proprietary AC1 data: no for minimized synthetic regressions/fuzzing.
- Verification status: key mechanisms independently source-verified at target SHA; no fuzz campaign executed. Downgraded from an initial higher rating because the demonstrated trigger is malformed/local data, not normal broad operation.

### [AC1-REV-015] Structurally plausible asset data can be accepted with ambiguous or invalid semantics

- Severity: Medium
- Confidence: High for parser behavior; Medium for occurrence in compatible shipped data
- Components: action graph, skeleton/skinning, resource/material/texture resolution, native transforms
- Exact code evidence: the asset-boundary inspection identifies unresolved ActionBlock refs/enums/cardinality and non-finite weights that can survive syntactic parsing; skeleton parent resolution can silently convert a missing parent to None or retain a later parent; several resource/material lookups scan arbitrary byte windows and accept first matches; transform validation does not consistently establish the stronger rigidity/skinning invariants downstream assumes.
- Failure mechanism: a decoder can return an apparently valid object whose references, hierarchy, transform or resource identity is wrong. The error then appears later as a wrong pose/material/collision result rather than a clean unsupported/corrupt-data boundary.
- Trigger/reproducer: byte-complete ActionBlock with duplicate/unresolved IDs or NaN weight; later-parent skeleton; decoy valid material ID before the real field; two plausible TextureMapSpec candidates; finite shear/near-singular transform.
- User-visible consequence: missing/overwritten actions, NaN pose arithmetic, wrong material/texture selection, silently re-rooted skeleton, or distorted/missing geometry.
- Recommended repair: separate syntactic decode from semantic validation; reject duplicate/ambiguous IDs, unresolved required refs, invalid enum/cardinality, non-finite values and unsupported transform domains. Parse typed fields where known; when scanning remains a PORT heuristic, require unique semantically valid candidates and preserve provenance.
- Regression test: corpus mutations preserving byte length but violating each semantic invariant; property that every Ok result satisfies the downstream contract.
- Requires proprietary AC1 data: no for synthetic regressions; real corpus is useful for compatibility validation.
- Verification status: supported by complete source inspection; selected decoder behaviors independently checked; normal shipped-data occurrence is not established.

### [AC1-REV-016] Process-global game-data/config caches are not scoped to an installation or App lifetime

- Severity: Medium
- Confidence: High
- Components: src/assets/mod.rs; src/assets/altair.rs; runtime configuration
- Exact code evidence: assets::GAME_DIR is OnceLock<Option<PathBuf>> at assets/mod.rs:26 and set_game_dir ignores a failed second set at 40. altair::SHADER_RESOURCES is a process-global OnceLock<Result<Vec<Resource>, String>> at altair.rs:148-149, not keyed by game directory or data identity and capable of caching an error.
- Failure mechanism: the first discovery/load can become sticky for the process. A repaired file or second installation cannot reliably replace cached state, and a later load can reuse shader resources from an earlier installation.
- Trigger/reproducer: first shader-resource load fails then data is repaired; or load install A then install B in one process; or initialize GAME_DIR before a later set_game_dir.
- User-visible consequence: persistent false failure or cross-install resource mixing during retries/tests/multi-App use.
- Recommended repair: put parsed external-data caches under an installation-scoped context keyed by canonical path plus build/file identity; parse effective runtime configuration once into an App resource. Avoid permanently caching transient failures unless startup-only immutability is the explicit contract.
- Regression test: two independent Apps/install contexts in one process plus fail-then-retry scenario.
- Requires proprietary AC1 data: no; temp/synthetic resources can establish isolation behavior.
- Verification status: independently source-verified; normal single-startup ordering avoids some paths, so Medium rather than High.

### [AC1-REV-017] Optional village-placement failure is not transactionally rolled back

- Severity: Low
- Confidence: High
- Components: src/native_map.rs optional placement / append_part
- Exact code evidence: the optional-placement recovery path saves/truncates collision and guidance lengths, while append_part may already have inserted decoded materials/textures before the Part is finally committed.
- Failure mechanism: a placement can decode/insert material A, fail on later material B, be skipped, and leave A resident even though the placement itself was rejected.
- Trigger/reproducer: optional two-material placement where first material is valid and a later dependency is corrupt.
- User-visible consequence: unused material/texture state and extra CPU/GPU memory/upload; diagnostics can label corruption as merely unsupported decoration.
- Recommended repair: decode an optional placement into a temporary PendingPart and commit all collision/guidance/material/mesh state atomically; classify unsupported, corrupt and missing-dependency failures separately.
- Regression test: failed optional placement leaves all map collections logically unchanged.
- Requires proprietary AC1 data: no
- Verification status: source-verified; not runtime-executed.

### [AC1-REV-018] Persisted game-directory configuration is relative-path fragile and platform-incomplete

- Severity: Low
- Confidence: High
- Components: src/assets/mod.rs
- Exact code evidence: set_game_dir persists dir.display().to_string() without canonicalization; a relative path is later interpreted relative to a future process CWD. AC_GAME_DIR is accepted before validation/fallback discovery. Automatic Steam discovery enumerates Windows locations but not ordinary Linux Steam/Proton locations, and basic validity checks only DataPC.forge.
- Failure mechanism: the stored path's meaning can change across launches; a stale environment override can suppress otherwise valid discovery; an incomplete install can be accepted too early.
- Trigger/reproducer: persist ./relative-install, change CWD, relaunch; or set invalid AC_GAME_DIR with an otherwise discoverable valid install.
- User-visible consequence: confusing install-not-found/incompatible-data behavior and unnecessary manual setup.
- Recommended repair: canonicalize and persist absolute paths; define legacy-relative resolution; validate environment override before suppressing fallback; separate install capability checks and add supported Linux discovery paths if Linux is intended.
- Regression test: temp directories with changed CWD, invalid env override plus valid discovered install, platform-specific discovery fixtures.
- Requires proprietary AC1 data: no
- Verification status: source-verified; portability impact not runtime-tested in this pass.

## Native-fidelity gaps

These are unresolved native comparisons or deliberate PORT approximations, not
confirmed defects merely because they differ from the original game.

### Strong but unverified risks

1. Collision/query layer fidelity. Several helper queries (point_inside,
   sphere_free_distance, capsule_fits/cast, ground/floor helpers) do not carry
   the richer query filter/layer semantics described by the recovered call
   sites, while the character proxy itself does. This can plausibly change
   transition predicates, but comments claiming a native layer number are not
   independent proof. Resolve the native filter words and then add
   layer-decoy fixtures before promoting this to a confirmed mismatch.
2. Transformed guidance normals. Placement transforms accept general affine
   forms while normal handling is not demonstrated correct for scale/shear.
   No evidence in the review established that relevant placements actually use
   those transforms.
3. Guidance/tie-break semantics. Signed wall/wall filtering, additional
   EdgeFilter bits, and first-match Beam/Ladder ordering need native
   collector/tie-break evidence.
4. Secondary visual ordering. IK and secondary visual modifiers are both
   ordered after animation, but no explicit relation is declared between them.
   Source proves the under-specified schedule, not which order the reviewed
   runtime actually chose in a concrete frame. Treat as
   architecture/validation risk until a production-schedule test demonstrates
   a wrong pose.

### Deliberate documented PORT approximations

- Ground event arbitration substitutes port triggers for untraced native event
  senders; branch priority may differ.
- Targetless jumping invents FREE_JUMP_DISTANCE even though repository
  comments state native resolves a target.
- Some NarrowObject handoffs are collapsed into Ground.
- Collision-layer ownership is context-level in the port rather than
  recovered native substate-level switching.
- Swing cancellation/release uses a substitute input/event model.
- Pass-over completion substitutes action-end timing for an unresolved native
  release point.
- Ledge/climb path clearance is partially approximated; where the omission
  permits interpolation through a synthetic solid blocker it is promoted
  above as a confirmed finding.
- point_inside uses a parity ray that assumes a closed authored triangle
  volume.
- capsule_cast_free is itself discretized at up to 5 cm steps.
- ground_height returns height rather than a full filtered support/contact
  with slope semantics.
- no-target lean decay uses a literal 0.1-per-update rule copied from the
  executable; the correct port cadence depends on what scheduling domain the
  original function ran in.

### Original-game evidence still required

- exact native ground event priority/arbitration;
- native query layer/filter words and collector ordering;
- exact substate collision-layer switching;
- swing/pass-over event timing;
- native cloth dispatch/task/LOD cadence, especially across hitches;
- Autodesk HumanIK end-effector orientation semantics;
- original secondary-modifier reset semantics on teleport/pause/stall;
- character material TextureGradient interpretation, eye cubemap basis,
  color-space behavior and environment-light contribution;
- native cloth normal/tangent solver behavior;
- AROLLB transition/root behavior where source-address comments alone are not
  enough to establish fidelity.

## Validation and enforcement gaps

These are enforcement/test gaps, not correctness findings by themselves.

### Zero-proprietary-data CI

Highest leverage:

1. Add a required toolchain/source gate:

       cargo fmt --check
       cargo check --locked --all-targets
       cargo test --locked
       cargo test --release --locked

   Pin the intended Rust toolchain/rust-version and require the resulting
   status before integration.
2. Build a headless harness from the same production gameplay/plugin
   registration rather than only the hand-maintained Sim chain. The current
   simulator omits at least proxy_layer/sync_visuals and the
   animation/IK/visual/map composition.
3. Add a small frame-rate/hitch matrix rather than certifying only fixed 60 Hz
   behavior.
4. Enable schedule ambiguity diagnostics in development and express semantic
   stages explicitly where order is required.

### Synthetic fixture tests

No proprietary data is required for the highest-risk regressions. Representative
fixtures include stale landing replay, shallow-incidence cast and
large-step/substep equivalence, authored angled wall normals, exact context-edge
activation matrices, interrupted climb continuation, the Fall velocity
contract, long-catch freshness, a frame-partition matrix, a mid-sample sphere
obstacle, four-plane cavity and insertion-order permutations, a blocked
traversal corridor, synthetic rolling blend and root tracks, a poisoned visual
reset epoch, parser truncation/count/semantic-property fuzz corpora, and a tiny
project-authored in-memory native-map/resource fixture that exercises resource
lookup -> mesh/collision/guidance assembly -> map installation without
redistributed game content.

### Real AC1-install tests

The current source frequently uses early return when the install is absent,
which makes cargo test report a successful test function without certifying the
integration path.

Add an explicit required-data mode such as AC_REQUIRE_GAME_DATA=1:

- missing/incompatible install: hard failure;
- valid install: execute actual Forge/model/animation/native-map/source-collision/guidance/ladder/beam/map-reset tests;
- verify the expected source-data/build identity before treating results as comparable;
- regenerate/diff native-derived tables where their generator is available.

A public zero-data job may still pass, but its report must say asset integration
was not certified.

### Original-game behavioural/visual comparison

Create controlled differential captures at equal simulation timestamps for
transition interruption; walling incidence and target choice; jump/landing/
reception classification; 30/60/120/144/180 Hz plus deterministic hitches; IK
reach/release and map reset; cloth dispatch/deformation; and fixed camera/light
material and eye-cubemap rendering. Capture Body state, action/item/phase/blend,
root transform and key joint transforms alongside images so a visual mismatch
can be traced to state rather than judged subjectively.

### Reproducibility and provenance

The existing bug recorder is useful but does not bind an input trace to all
environment state needed for an exact replay. Add a versioned manifest with
source commit/build identity; active map and lifecycle events; effective
runtime configuration; game-data/build identity or fingerprints; and initial
locomotion/context state. Promote minimized/sanitized regressions from ignored
local bugs into a committed test corpus.

Native-source comments are well labelled, but the public tree does not contain
the cited reverse-engineering material. Add a provenance index that
distinguishes public/reproducible evidence from unpublished evidence and binds
generated native-derived files to generator commands/input identity/output
hashes.

## Checked non-findings

Important negative results from the review are preserved here.

### Movement/state

- release_limbs clears hand/foot targets outside Ledge/Climb; no ordinary
  context-exit path was found that leaves logical IK targets pinned.
- HumanLedgeData entry resets the important transient move/queue/pending state;
  deferred NaN grab destination is intentionally filled before interpolation.
- no demonstrated stale continuation comparable to Climb.to_ground was found in
  Walling/Narrow/Ladder local action state.
- audited jump/ledge/ladder/blend paths generally use normalize_or*, explicit
  zero checks or minimum denominators; no concrete ordinary NaN/zero-vector
  transition bug was established.
- ITEM_FLAGS is correctly sorted for its binary search.
- duplicate 0x023e0c60 swing-cycle data is equivalent; first-match lookup does
  not change behavior.
- stale Air takeoff/flight fields were inspected but no erroneous Fall behavior
  was established.

### Collision/spatial

- imported authored collision is not generally reduced to world AABBs: rotated
  boxes are converted to triangles and mesh collision remains triangle
  geometry.
- no concrete triangle broadphase-culling false-negative was established.
- persistent proxy manifold across successful map switching was not found.
- no demonstrated stale-static manifold bug was found after context layer
  switching.
- ordinary two-plane corner handling is explicitly supported; the concrete
  solver defect is the four-plane fallback.
- Triangle::new rejects non-finite/extremely degenerate triangles; there is no
  unconditional degenerate-triangle acceptance bug.
- using total displacement length in conservative advancement is not inherently
  non-conservative per step; the confirmed defect is specifically the
  finite-iteration exhaustion -> no-hit failure.
- no current large-coordinate precision bug was established.

### Animation/visual

- limb handedness/index ordering is consistent across reviewed
  targets/chains/contact bits.
- normal finite-input two-bone IK math has meaningful guards/tests; no direct
  analytic NaN path was identified.
- cloth contact math already covers degenerate/parallel cases reasonably.
- cloth world/model-space conversion did not reveal an obvious inversion.
- secondary visual target/bone validation is comparatively strong.
- normal/specular/ramp resources use linear texture formats while diffuse uses
  sRGB; no straightforward normal-map-as-sRGB defect was found.
- inner/outer cloth intentionally share one deforming mesh rather than running
  duplicate solvers.
- material parser already rejects several truncation/non-finite corruption
  cases.

### Asset/lifecycle

- parse_mesh/static/collision mesh and guidance readers contain significant
  existing range/cardinality/finite-value validation; the review does not
  classify every unwrap as unsafe.
- failed native-map decode occurs before teardown, so an ordinary failed import
  preserves the previous scene.
- successful repeated map switches replace collision/guidance and core player
  state; no evidence was found that old CollisionWorld/GuidanceWorld remains
  installed.
- native-map Resources.cache is per-load rather than the problematic
  process-global cache.
- optional missing character/animation assets and several import errors are
  generally surfaced/recoverable; the codebase is not accurately described as
  having globally poor error handling.

## Overall verdict

Structurally strong: the movement simulation corpus is large and targeted;
reverse-engineering confidence is usually labelled with
PORT/PLACEHOLDER/hypothesis/source-address distinctions rather than hidden;
many low-level geometry, IK, material and parser paths already have meaningful
guards/tests; and failed map import is intentionally transactional at the
large-scene level.

Presently fragile: transition data lifetime is not consistently event-owned;
collision/query approximations can change state decisions, including two High
spatial failures; elapsed-time semantics diverge across interacting systems;
visual reset/blend state crosses boundaries it should not; external data
decoding has several trust-before-validation and semantic-ambiguity surfaces;
and test breadth is not backed by an equally strong production-schedule, CI and
real-install enforcement envelope.

The baseline is not a failed rewrite: its test corpus and explicit
approximation markers are useful foundations. It is, however, not yet safe to
treat broad simulator success as proof of native-correct production behavior,
and the High-severity findings should be repaired before deeper fidelity work
is used as evidence of correctness.
