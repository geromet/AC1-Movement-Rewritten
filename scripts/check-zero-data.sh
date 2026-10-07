#!/usr/bin/env bash
#
# Zero-data validation boundary for AC1 Movement Rewritten (ac_port).
#
# This script is the single enforcement point for what a green zero-data run
# does and does not certify. The full contract is in docs/VALIDATION.md.
#
# It runs `cargo check` and `cargo test`, aggregates the test result across
# every test binary, asserts that no test failed, and surfaces the ignored and
# install-gated skip inventory instead of letting those tests masquerade as
# exercised native-data tests.
#
# Modes
#   (default)               zero-data: no AC1 install is required. The
#                           install-gated tests must skip, and the skip/ignore
#                           inventory is asserted and printed.
#   AC_REQUIRE_GAME_DATA=1  required-data: a valid install MUST be present and
#                           no install-gated test may skip, otherwise the run
#                           hard-fails instead of silently passing.
#
# Environment
#   AC_GAME_DIR             user's own AC1 install directory (contains
#                           DataPC.forge). Used for install detection.
#   AC_REQUIRE_GAME_DATA    1 => required-data mode (hard-fail if absent).
#   AC_OFFLINE              1 => pass --offline to cargo (air-gapped/local).
#   AC_SKIP_CHECK           1 => do not run `cargo check` (caller already did).
#   AC_TEST_LOG             path to a captured `cargo test -- --nocapture` log;
#                           when set, `cargo test` is not re-run.
#   AC_EXPECT_INSTALL_GATED expected count of 'skipped: game install not found'
#                           (default 13; set to empty or 0 to disable).
#   AC_EXPECT_IGNORED       expected #[ignore] count (default 21; empty/0
#                           disables).
#
# No proprietary AC1 data, no secrets, no network fetch of game data.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CARGO_ARGS=(--locked)
if [[ "${AC_OFFLINE:-0}" == "1" ]]; then
  CARGO_ARGS+=(--offline)
fi

install_present() {
  local dir="${AC_GAME_DIR:-}"
  [[ -n "$dir" && -f "$dir/DataPC.forge" ]]
}

REQUIRE="${AC_REQUIRE_GAME_DATA:-0}"
EXPECT_SKIPS="${AC_EXPECT_INSTALL_GATED-13}"
EXPECT_IGNORED="${AC_EXPECT_IGNORED-21}"

if install_present; then
  INSTALL_STATE="present (AC_GAME_DIR=${AC_GAME_DIR})"
else
  INSTALL_STATE="absent"
fi

echo "== AC1 zero-data validation boundary =="
echo "repo:       $ROOT"
echo "install:    $INSTALL_STATE"
echo "mode:       $( [[ "$REQUIRE" == "1" ]] && echo required-data || echo zero-data )"
echo "cargo args: ${CARGO_ARGS[*]}"
echo

if [[ "$REQUIRE" == "1" && "$INSTALL_STATE" == absent ]]; then
  echo "FATAL: AC_REQUIRE_GAME_DATA=1 but no valid AC1 install was found." >&2
  echo "       Set AC_GAME_DIR to a directory containing DataPC.forge." >&2
  echo "       Refusing to report success without exercising native-data tests." >&2
  exit 3
fi

if [[ "${AC_SKIP_CHECK:-0}" != "1" ]]; then
  echo "-- cargo check ${CARGO_ARGS[*]} --"
  cargo check "${CARGO_ARGS[@]}"
  echo
fi

TEST_LOG="${AC_TEST_LOG:-}"
CLEANUP_LOG=""
if [[ -z "$TEST_LOG" ]]; then
  TEST_LOG="$(mktemp)"
  CLEANUP_LOG="$TEST_LOG"
  trap 'rm -f "$CLEANUP_LOG"' EXIT
  echo "-- cargo test ${CARGO_ARGS[*]} -- --nocapture --test-threads=1 --"
  set +e
  cargo test "${CARGO_ARGS[@]}" -- --nocapture --test-threads=1 2>&1 | tee "$TEST_LOG"
  TEST_EXIT=${PIPESTATUS[0]}
  set -e
  echo
elif [[ ! -f "$TEST_LOG" ]]; then
  echo "FATAL: AC_TEST_LOG=$TEST_LOG does not exist" >&2
  exit 2
fi

# Aggregate passed/failed/ignored across every test binary (unit + integration
# + doc tests), so a later binary cannot hide an earlier failure.
read -r PASSED FAILED IGNORED < <(
  awk '
    /^test result:/ {
      for (i = 1; i <= NF; i++) {
        if ($i == "passed;")  p += $(i-1);
        if ($i == "failed;")  f += $(i-1);
        if ($i == "ignored;") g += $(i-1);
      }
    }
    END { printf "%d %d %d\n", p, f, g }
  ' "$TEST_LOG"
)

if ! grep -q '^test result:' "$TEST_LOG"; then
  echo "FATAL: no 'test result:' line found in $TEST_LOG (build failure?)" >&2
  exit 2
fi

SKIPPED="$(grep -c 'skipped: game install not found' "$TEST_LOG" || true)"
SKIPPED_ABSENT="$(grep -c 'skipped: game install absent' "$TEST_LOG" || true)"

echo "inventory: passed=$PASSED failed=$FAILED ignored=$IGNORED"
echo "           install-gated skipped=$SKIPPED (+${SKIPPED_ABSENT} 'game install absent')"

fail=0
if [[ "$FAILED" != "0" ]]; then
  echo "FAIL: $FAILED test(s) failed" >&2
  fail=1
fi

if [[ "$INSTALL_STATE" == absent ]]; then
  if [[ "$SKIPPED" == "0" ]]; then
    echo "FAIL: no install is present but no install-gated test reported a skip." >&2
    echo "      The skip path is not being exercised; the boundary cannot be trusted." >&2
    fail=1
  fi
  if [[ -n "$EXPECT_SKIPS" && "$EXPECT_SKIPS" != "0" && "$SKIPPED" != "$EXPECT_SKIPS" ]]; then
    echo "FAIL: expected $EXPECT_SKIPS install-gated skips, saw $SKIPPED." >&2
    echo "      Update AC_EXPECT_INSTALL_GATED if the inventory changed intentionally." >&2
    fail=1
  fi
else
  if [[ "$SKIPPED" != "0" ]]; then
    echo "FAIL: an install is present but $SKIPPED install-gated test(s) still skipped." >&2
    fail=1
  fi
fi

if [[ -n "$EXPECT_IGNORED" && "$EXPECT_IGNORED" != "0" && "$IGNORED" != "$EXPECT_IGNORED" ]]; then
  echo "FAIL: expected $EXPECT_IGNORED ignored tests, saw $IGNORED." >&2
  echo "      Update AC_EXPECT_IGNORED if the inventory changed intentionally." >&2
  fail=1
fi

if [[ "$REQUIRE" == "1" && "$SKIPPED" != "0" ]]; then
  echo "FAIL: required-data mode but $SKIPPED install-gated test(s) skipped." >&2
  fail=1
fi

if [[ "$fail" != "0" ]]; then
  echo "RESULT: FAIL" >&2
  exit 1
fi

cat <<EOF

RESULT: PASS

A green run certifies:
  - the source compiles (cargo check) and the synthetic/source test suite
    passes with zero failures;
  - the ignored/install-gated inventory is exactly as declared, so skips are
    surfaced rather than counted as exercised native-data tests.

A green run does NOT certify:
  - asset integration against a real AC1 install;
  - native parity of movement/animation/decoding against the retail game;
  - anything about proprietary data, which is never bundled or fetched.

Ignored tests ($IGNORED) and install-gated skips ($SKIPPED) were NOT exercised.
Set AC_REQUIRE_GAME_DATA=1 with AC_GAME_DIR to require real install coverage.
EOF
