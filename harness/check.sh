#!/usr/bin/env bash
# Quality gate for every agent handoff. Usage: ./harness/check.sh [--fast]
#   --fast  skips the slow integration suites (sk_concurrency, sk_performance), the tests
#           that wait out a fixed limit (10 s JS cap, ...) and the no-plugins test run
#   STRICT=0 ./harness/check.sh  reports clippy/fmt without failing (opt-out)
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n==> %s\n' "$*"; }

step "build (default features)"
cargo build --all-targets --quiet

step "build (no plugins: lib, bins, tests)"
cargo build --all-targets --no-default-features --quiet

step "tests"
if [[ "${1:-}" == "--fast" ]]; then
  # Skipped: the sk_concurrency / sk_performance suites, and the tests that wait out a fixed
  # wall-clock limit (BL-087, QA-7): the 10 s JS per-call cap (2 tests), the 2 s server
  # deadline on a JS hook, and the 3 s "daemon does not start" window. CI runs them all.
  cargo test --quiet -- --skip test_concurrent_ --skip test_stress_ --skip test_resource_cleanup \
    --skip test_benchmark_ --skip test_memory_usage_ \
    --skip per_call_cap_stops_a_looping_hook --skip upload_with_a_looping_example \
    --skip server_deadline_stops_a_looping_hook --skip pid_file_through_a_symlink
else
  cargo test --quiet
  # BL-087 (QA-3): the no-plugins build is an invariant (AGENTS.md section 7); this runs its
  # behaviour too (the no-plugins HTTP paths, the JS-free limits). Full mode only.
  step "tests (no plugins)"
  cargo test --no-default-features --quiet
fi

# STRICT=1 (default): clippy -D warnings (default and no-plugins) + fmt --check.
# STRICT=0 is an opt-out for local experiments only; handoffs require the strict gate.
STRICT="${STRICT:-1}"
step "clippy + fmt (strict=$STRICT)"
if [[ "$STRICT" == "1" ]]; then
  cargo clippy --all-targets --quiet -- -D warnings
  cargo clippy --all-targets --no-default-features --quiet -- -D warnings
  cargo fmt --check
else
  cargo clippy --all-targets --quiet || true
  cargo fmt --check || true
fi

step "OK"
