#!/usr/bin/env bash
# Quality gate for every agent handoff. Usage: ./harness/check.sh [--fast]
#   --fast  skips the slow integration suites (sk_concurrency, sk_performance) and the
#           no-plugins test run
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
  cargo test --quiet -- --skip test_concurrent_ --skip test_stress_ --skip test_resource_cleanup \
    --skip test_benchmark_ --skip test_memory_usage_
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
