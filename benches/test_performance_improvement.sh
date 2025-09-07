#!/bin/bash

# Simple test to demonstrate performance improvement
set -e

echo "🚀 Testing Skillet Performance Improvement"
echo "=========================================="

# Build if needed
if [ ! -f "target/release/sk_server" ] || [ ! -f "target/release/sk" ]; then
    echo "Building release binaries..."
    cargo build --release --bin sk --bin sk_server --bin sk_client
fi

echo ""
echo "1. Testing original sk command (process-per-request):"
echo "   Expression: =2 + 3 * 4"

# Test original method (5 iterations to show consistency)
echo "   Running 5 iterations..."
for i in {1..5}; do
    start=$(date +%s.%N)
    result=$(./target/release/sk "=2 + 3 * 4" 2>/dev/null || echo "failed")
    end=$(date +%s.%N)
    duration=$(echo "$end - $start" | bc -l)
    printf "   Iteration %d: %.0fms (result: %s)\n" $i $(echo "$duration * 1000" | bc -l) "$result"
done

echo ""
echo "2. Testing server mode (keep interpreter in memory):"

# Start server in background
echo "   Starting server on port 8888..."
./target/release/sk_server 8888 4 > /dev/null 2>&1 &
SERVER_PID=$!

# Wait for server startup
sleep 1

# Cleanup function
cleanup() {
    if kill -0 $SERVER_PID 2>/dev/null; then
        kill $SERVER_PID
        wait $SERVER_PID 2>/dev/null || true
    fi
}
trap cleanup EXIT

# Test server method
echo "   Running 5 iterations via server..."
for i in {1..5}; do
    start=$(date +%s.%N)
    result=$(./target/release/sk_client localhost:8888 "=2 + 3 * 4" 2>/dev/null | tr -d '"' || echo "failed")
    end=$(date +%s.%N)
    duration=$(echo "$end - $start" | bc -l)
    printf "   Iteration %d: %.0fms (result: %s)\n" $i $(echo "$duration * 1000" | bc -l) "$result"
done

echo ""
echo "3. Quick benchmark test (100 operations):"
echo "   Running benchmark via sk_client..."
./target/release/sk_client localhost:8888 --benchmark "=2 + 3 * 4" 100

echo ""
echo "✅ Performance test completed!"
echo ""
echo "Expected results:"
echo "  • Original method: ~200-300ms per operation"
echo "  • Server method: ~1-10ms per operation"  
echo "  • Improvement: 20-300x faster"