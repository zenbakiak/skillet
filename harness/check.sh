#!/usr/bin/env bash
# Quality gate for every agent handoff. Usage: ./harness/check.sh [--fast]
#   --fast  skips the slow integration suites (sk_concurrency, sk_performance)
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n==> %s\n' "$*"; }

step "build (default features)"
cargo build --all-targets --quiet

step "build (no plugins)"
cargo build --lib --no-default-features --quiet

step "tests"
if [[ "${1:-}" == "--fast" ]]; then
  cargo test --quiet -- --skip test_concurrent_ --skip test_stress_ --skip test_resource_cleanup \
    --skip test_benchmark_ --skip test_memory_usage_
else
  cargo test --quiet
fi

# ponytail: clippy/fmt are report-only until BL-003 lands; then flip STRICT=1 permanently.
STRICT="${STRICT:-0}"
step "clippy (strict=$STRICT)"
if [[ "$STRICT" == "1" ]]; then
  cargo clippy --all-targets --quiet -- -D warnings
  cargo fmt --check
else
  n=$(cargo clippy --all-targets --quiet 2>&1 | grep -c '^warning' || true)
  echo "clippy warnings: $n (baseline after BL-001: 92; must not increase)"
fi

step "OK"
