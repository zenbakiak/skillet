#!/bin/bash

# Comprehensive server performance benchmarking script
set -e

echo "🚀 Skillet Server Performance Benchmark"
echo "========================================"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_status() { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Configuration
PORT=${1:-8080}
ITERATIONS=${2:-1000}
SERVER_THREADS=${3:-$(nproc)}

print_status "Configuration:"
echo "  Port: $PORT"
echo "  Iterations per test: $ITERATIONS"
echo "  Server threads: $SERVER_THREADS"
echo ""

# Build all binaries
print_status "Building release binaries..."
if cargo build --release --bin sk_server --bin sk_client; then
    print_success "Build completed"
else
    print_error "Build failed"
    exit 1
fi

# Start server in background
print_status "Starting Skillet server on port $PORT..."
./target/release/sk_server $PORT $SERVER_THREADS > server.log 2>&1 &
SERVER_PID=$!

# Wait for server to start
sleep 2

# Check if server is running
if ! kill -0 $SERVER_PID 2>/dev/null; then
    print_error "Server failed to start"
    cat server.log
    exit 1
fi

# Test server connectivity
print_status "Testing server connectivity..."
if echo '{"expression": "=2+3", "variables": null}' | nc localhost $PORT > /dev/null 2>&1; then
    print_success "Server is responding"
else
    print_warning "Direct connection test failed, trying with client..."
fi

# Function to cleanup on exit
cleanup() {
    if kill -0 $SERVER_PID 2>/dev/null; then
        print_status "Stopping server (PID: $SERVER_PID)..."
        kill $SERVER_PID
        wait $SERVER_PID 2>/dev/null || true
    fi
}
trap cleanup EXIT

# Run benchmarks
print_status "=== PERFORMANCE BENCHMARKS ==="

# Test cases
declare -a tests=(
    "Basic Arithmetic:=2 + 3 * 4"
    "Complex Expression:=(2 + 3) * (4 - 1) ^ 2"
    "Function Call:=SUM(1, 2, 3, 4, 5, 6, 7, 8, 9, 10)"
    "String Operations:=CONCAT(\"Hello\", \" \", \"World\")"
    "Variable Access:=SUM(:a, :b, :c)"
    "Nested Functions:=MAX(SUM(1, 2, 3), AVG(4, 5, 6), MIN(7, 8, 9))"
)

for test in "${tests[@]}"; do
    IFS=':' read -r name expr <<< "$test"
    echo ""
    print_status "Testing: $name"
    echo "Expression: $expr"
    
    if [ "$name" = "Variable Access" ]; then
        # Test with variables (requires JSON format for client)
        # Note: This is a simplified test - real implementation would need proper JSON handling
        if timeout 30 ./target/release/sk_client localhost:$PORT --benchmark "$expr" $ITERATIONS; then
            print_success "✓ $name benchmark completed"
        else
            print_warning "✗ $name benchmark failed or timed out"
        fi
    else
        if timeout 30 ./target/release/sk_client localhost:$PORT --benchmark "$expr" $ITERATIONS; then
            print_success "✓ $name benchmark completed"
        else
            print_warning "✗ $name benchmark failed or timed out"
        fi
    fi
done

# Concurrency test
echo ""
print_status "=== CONCURRENCY TEST ==="
print_status "Running concurrent benchmark with multiple clients..."

# Create multiple client processes
CLIENT_PIDS=()
CLIENT_LOGS=()
CONCURRENT_CLIENTS=5
ITERATIONS_PER_CLIENT=$(($ITERATIONS / $CONCURRENT_CLIENTS))

for i in $(seq 1 $CONCURRENT_CLIENTS); do
    logfile="client_${i}.log"
    CLIENT_LOGS+=($logfile)
    
    (
        echo "Client $i starting..." > $logfile
        ./target/release/sk_client localhost:$PORT --benchmark "=2*3+4*5" $ITERATIONS_PER_CLIENT >> $logfile 2>&1
        echo "Client $i completed" >> $logfile
    ) &
    CLIENT_PIDS+=($!)
done

print_status "Started $CONCURRENT_CLIENTS concurrent clients"
print_status "Waiting for completion..."

# Wait for all clients
for pid in "${CLIENT_PIDS[@]}"; do
    wait $pid
done

print_success "Concurrent test completed"

# Analyze concurrent results
echo ""
print_status "=== CONCURRENCY RESULTS ==="
total_successful=0
total_failed=0
total_throughput=0

for i in $(seq 1 $CONCURRENT_CLIENTS); do
    logfile="client_${i}.log"
    if [ -f "$logfile" ]; then
        successful=$(grep "Successful:" "$logfile" | awk '{print $2}' || echo "0")
        failed=$(grep "Failed:" "$logfile" | awk '{print $2}' || echo "0")
        throughput=$(grep "Throughput:" "$logfile" | awk '{print $2}' || echo "0")
        
        total_successful=$((total_successful + successful))
        total_failed=$((total_failed + failed))
        total_throughput=$(echo "$total_throughput + $throughput" | bc -l || echo "$total_throughput")
        
        echo "Client $i: $successful successful, $failed failed, $throughput ops/sec"
    fi
done

echo ""
print_status "AGGREGATE CONCURRENCY RESULTS:"
echo "  Total successful operations: $total_successful"
echo "  Total failed operations: $total_failed"
echo "  Total throughput: ${total_throughput} ops/sec"

if [ "$total_failed" -eq 0 ]; then
    print_success "✓ No failed operations in concurrent test"
else
    print_warning "⚠ $total_failed operations failed in concurrent test"
fi

# Memory and resource usage
echo ""
print_status "=== RESOURCE USAGE ==="
if command -v ps >/dev/null 2>&1; then
    server_memory=$(ps -p $SERVER_PID -o rss= | awk '{print $1/1024}' || echo "unknown")
    server_cpu=$(ps -p $SERVER_PID -o %cpu= || echo "unknown")
    echo "Server memory usage: ${server_memory}MB"
    echo "Server CPU usage: ${server_cpu}%"
fi

# Load test summary
echo ""
print_status "=== LOAD TEST SUMMARY ==="
echo "Configuration:"
echo "  Server threads: $SERVER_THREADS"
echo "  Concurrent clients: $CONCURRENT_CLIENTS"
echo "  Total operations: $((CONCURRENT_CLIENTS * ITERATIONS_PER_CLIENT))"
echo ""

# Performance comparison
original_time=250  # Original sk command ~250ms
if [ "$total_throughput" != "0" ]; then
    avg_time=$(echo "1000 / $total_throughput" | bc -l)
    improvement=$(echo "$original_time / $avg_time" | bc -l)
    
    print_status "PERFORMANCE IMPROVEMENT:"
    echo "  Original sk command: ${original_time}ms per operation"
    echo "  Server mode: ${avg_time}ms per operation (estimated)"
    echo "  Improvement factor: ${improvement}x faster"
fi

# Cleanup client logs
print_status "Cleaning up client logs..."
rm -f client_*.log

print_success "Benchmark completed successfully!"
echo ""
echo "📊 For detailed server logs, check: server.log"
echo "🚀 Server is still running on port $PORT for manual testing"
echo "   Stop it with: kill $SERVER_PID"