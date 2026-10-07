# Validation contract — zero-data boundary

AC1 Movement Rewritten (`ac_port`) is a clean-room recreation that loads the
animations and character from **your own copy of the game at runtime**. No
proprietary Assassin's Creed data is bundled, fetched, or required to build.
This document defines the public validation boundary and, importantly, the
limit of what a green run proves.

The boundary has two machine-enforced entry points:

- `scripts/check-zero-data.sh` — the local runner and enforcement point.
- `.github/workflows/ci.yml` — the public CI job, which runs `cargo check` and
  `cargo test` and then invokes the same script on the captured test log.

Both use only `cargo`, standard POSIX tools, and no secrets.

## Modes

### Zero-data mode (default)

No game install is required. The install-gated tests take their documented
`skipped: game install not found` path and the ignored tests are not run. The
boundary **asserts that this skipping actually happened** and that the counts
match the declared inventory, so a green run cannot silently count a skipped
native-data test as exercised.

```sh
./scripts/check-zero-data.sh
```

### Required-data mode (`AC_REQUIRE_GAME_DATA=1`)

A valid install **must** be present, and no install-gated test may skip.
Otherwise the boundary hard-fails instead of reporting a false green.

```sh
AC_REQUIRE_GAME_DATA=1 AC_GAME_DIR="/path/to/Assassin's Creed" \
  ./scripts/check-zero-data.sh
```

Install detection for this boundary is `AC_GAME_DIR/DataPC.forge`. This is a
deliberate subset of the application's full `find_game_dir` search (which also
probes `game_dir.txt`, ancestor folders, and Steam libraries); set
`AC_GAME_DIR` explicitly when using required-data mode.

## The ladder

| Step | Command | Purpose |
| --- | --- | --- |
| 1 | `cargo check --locked` | The source compiles with the committed lockfile. |
| 2 | `cargo test --locked -- --nocapture --test-threads=1` | The full synthetic/source suite runs and its skips are visible. |
| 3 | aggregate `test result:` across every binary | Fail if any test binary reports a failure. |
| 4 | assert skip/ignore inventory | Skips and ignores are surfaced and cannot masquerade as native coverage. |
| 5 | required-data hard-fail | With `AC_REQUIRE_GAME_DATA=1`, refuse to pass if an install is absent or any install-gated test skipped. |

The script aggregates `passed`/`failed`/`ignored` across all test binaries
(unit tests plus integration tests) so a later binary cannot hide an earlier
failure.

## What a green zero-data run certifies

- The source compiles against the committed `Cargo.lock`.
- The synthetic/source test suite passes with **zero failures**.
- The decoder, movement, and animation invariants that are testable without
  retail data hold.
- The ignored/install-gated inventory is exactly as declared, so the run is
  **truthful about what it did not exercise**.

## What a green zero-data run does NOT certify

- **Asset integration.** Nothing was decoded from a real AC1 `DataPC*.forge`.
- **Native parity.** Behaviour was not compared against the retail game.
- **Native-data test coverage.** The ignored and install-gated tests did not
  run; they are reported as skipped, not passed.
- Anything about proprietary data, which is never bundled or fetched.

Use `AC_REQUIRE_GAME_DATA=1` with `AC_GAME_DIR` when you need the boundary to
prove that real install coverage actually executed.

## Declared inventory (base `28a4cbafa8af7873a8331c35d7a17d936c1dc48d`)

| Category | Count | Notes |
| --- | --- | --- |
| `#[ignore]` tests | 21 | 11 in `src/assets/probe.rs`, 10 in `src/sim_tests.rs`. Not run by default. |
| Install-gated `skipped: game install not found` | 13 | 10 in `src/assets/tests.rs`, 2 in `src/anim.rs`, 1 in `src/assets/world.rs`. |
| Install-absent (other strings) | 2 | `src/player/narrow.rs` (`game install absent`), `src/assets/world.rs` (`Masyaf archive absent`). |
| Native-map silent-return fixtures | 8 in `src/sim_tests.rs` + 2 in `src/native_map.rs` | Early-return when `native_map::simulation_fixture()`/`village_simulation_fixture()` is `None`; one `sim_tests.rs` fixture is also `#[ignore]`. |
| `#[test]` annotations | 269 | 248 executed + 21 ignored on this base. |

The expected counts are enforced via `AC_EXPECT_IGNORED` (default `21`) and
`AC_EXPECT_INSTALL_GATED` (default `13`). When the inventory changes
intentionally, update the defaults in `scripts/check-zero-data.sh` and this
table together.

## Environment reference

| Variable | Effect |
| --- | --- |
| `AC_GAME_DIR` | User's AC1 install directory (contains `DataPC.forge`). |
| `AC_REQUIRE_GAME_DATA=1` | Required-data mode: hard-fail if no install or if any install-gated test skipped. |
| `AC_OFFLINE=1` | Pass `--offline` to cargo for air-gapped/local runs. |
| `AC_SKIP_CHECK=1` | Do not run `cargo check` (the caller already did). |
| `AC_TEST_LOG` | Parse a captured `cargo test -- --nocapture` log instead of re-running tests. |
| `AC_EXPECT_INSTALL_GATED` | Expected install-gated skip count (default `13`; empty/`0` disables). |
| `AC_EXPECT_IGNORED` | Expected `#[ignore]` count (default `21`; empty/`0` disables). |

## Non-goals

- This boundary is not a fuzz harness, a benchmark, or a remote-security claim.
- It does not download game data or require network access.
- It does not replace the existing test suite; it adds enforcement around it.
