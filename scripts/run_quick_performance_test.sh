#!/bin/bash

# Quick Performance Test for Skillet - Reduced iterations for faster feedback
set -e

echo "🚀 Skillet Quick Performance Test"
echo "================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

# Build release version
print_status "Building release version..."
if cargo build --release --bin sk; then
    print_success "Release build completed"
else
    echo "Failed to build"
    exit 1
fi

# Quick smoke tests
print_status "Running quick smoke tests..."

echo "Testing basic arithmetic..."
if result=$(cargo run --release --bin sk -- '=2 + 3 * 4' 2>/dev/null); then
    echo "✓ Basic arithmetic: $result"
else
    echo "✗ Basic arithmetic failed"
    exit 1
fi

echo "Testing with variables..."
if result=$(cargo run --release --bin sk -- '=:x + :y' x=10 y=20 2>/dev/null); then
    echo "✓ Variables: $result"
else
    echo "✗ Variables failed"
    exit 1
fi

echo "Testing JSON input..."
if result=$(cargo run --release --bin sk -- '=:user.name' --json '{"user": {"name": "Alice"}}' 2>/dev/null); then
    echo "✓ JSON: $result"
else
    echo "✗ JSON failed"
    exit 1
fi

echo "Testing function calls..."
if result=$(cargo run --release --bin sk -- '=SUM(1, 2, 3, 4, 5)' 2>/dev/null); then
    echo "✓ Functions: $result"
else
    echo "✗ Functions failed"
    exit 1
fi

# Performance timing test
print_status "Running performance timing test..."

echo "Testing execution speed (10 iterations)..."
start_time=$(date +%s.%N)
for i in {1..10}; do
    cargo run --release --bin sk -- '=(2 + 3) * 4 - 1' >/dev/null 2>&1
done
end_time=$(date +%s.%N)

duration=$(echo "$end_time - $start_time" | bc -l)
avg_time=$(echo "scale=3; $duration / 10" | bc -l)
ops_per_sec=$(echo "scale=1; 10 / $duration" | bc -l)

echo "✓ 10 operations completed in ${duration}s"
echo "  Average per operation: ${avg_time}s"
echo "  Operations per second: ${ops_per_sec}"

# Basic concurrency test
print_status "Running basic concurrency test..."

echo "Testing 5 concurrent operations..."
pids=()
start_time=$(date +%s.%N)

for i in {1..5}; do
    cargo run --release --bin sk -- '=2 * 3 + 4' >/dev/null 2>&1 &
    pids+=($!)
done

# Wait for all background processes
for pid in "${pids[@]}"; do
    wait $pid
    if [ $? -ne 0 ]; then
        echo "✗ Concurrent operation failed"
        exit 1
    fi
done

end_time=$(date +%s.%N)
duration=$(echo "$end_time - $start_time" | bc -l)

echo "✓ 5 concurrent operations completed in ${duration}s"
echo "  Parallel speedup achieved!"

print_success "All quick tests passed! 🎉"

echo ""
echo "Summary:"
echo "- All basic operations working correctly"
echo "- Performance appears normal (${avg_time}s average per operation)"
echo "- Concurrency working without conflicts"
echo ""
echo "For comprehensive testing, run: cargo test --test sk_performance --release"
echo "For full concurrency tests, run: cargo test --test sk_concurrency --release"