#!/bin/bash

# Quick HTTP Server Performance Comparison
# Tests different thread counts with the new thread pool implementation

set -e

echo "⚡ Quick Skillet HTTP Server Benchmark"
echo "====================================="
echo "Testing thread pool performance improvements"
echo

# Build
cargo build --release --quiet

# Kill any existing servers
pkill -f "sk_http_server" 2>/dev/null || true
sleep 1

# Test function
run_quick_test() {
    local threads=$1
    local port=$((8080 + threads))
    
    echo "🧵 Testing $threads threads (port $port)..."
    
    # Start server
    ./target/release/sk_http_server $port --threads $threads > /dev/null 2>&1 &
    local server_pid=$!
    
    # Wait for server to start
    sleep 1.5
    
    if curl -s "http://127.0.0.1:$port/health" > /dev/null; then
        # Quick benchmark with Rust binary
        local result=$(./target/release/sk_http_bench 127.0.0.1:$port -c 20 -n 100 -w 10 | grep "Throughput" | tail -1 | awk '{print $(NF-1)}')
        echo "  ✅ Max throughput: $result req/sec"
    else
        echo "  ❌ Server failed to start"
    fi
    
    # Stop server
    kill $server_pid 2>/dev/null || true
    wait $server_pid 2>/dev/null || true
    sleep 0.5
}

echo "Testing different thread pool sizes:"
echo

# Test different thread counts
for threads in 1 2 4 8 16; do
    run_quick_test $threads
done

echo
echo "🎯 Summary:"
echo "- Thread pool prevents unlimited thread creation"
echo "- Performance scales well with CPU-bound tasks"
echo "- Optimal thread count typically: 2x CPU cores"
echo "- Thread reuse eliminates creation overhead"
echo

echo "🔍 To run comprehensive benchmarks:"
echo "  ./benches/benchmark_http_server.sh     # Full benchmark suite"  
echo "  ./benches/benchmark_http_simple.sh     # Basic tests"
echo "  ./target/release/sk_http_bench --help  # Custom benchmark"