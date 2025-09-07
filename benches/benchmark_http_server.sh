#!/bin/bash

# Skillet HTTP Server Performance Benchmark
# Tests throughput, latency, and concurrency performance

set -e

echo "=== Skillet HTTP Server Performance Benchmark ==="
echo "Date: $(date)"
echo "System: $(uname -a)"
echo

# Configuration
DEFAULT_PORT=8080
SERVER_HOST="127.0.0.1"
WARMUP_REQUESTS=50
TEST_DURATION=30  # seconds
CONCURRENT_LEVELS=(1 5 10 20 50 100)
THREAD_COUNTS=(1 2 4 8 16 32)

# Build optimized release version
echo "🔧 Building optimized release version..."
cargo build --release --quiet
echo "✅ Build complete"
echo

# Check dependencies
check_dependencies() {
    local missing_deps=()

    if ! command -v curl &> /dev/null; then
        missing_deps+=("curl")
    fi

    if ! command -v jq &> /dev/null; then
        missing_deps+=("jq")
    fi

    if ! command -v ab &> /dev/null && ! command -v wrk &> /dev/null; then
        missing_deps+=("apache2-utils (for 'ab') or wrk")
    fi

    if [ ${#missing_deps[@]} -ne 0 ]; then
        echo "❌ Missing dependencies: ${missing_deps[*]}"
        echo "Please install missing dependencies and run again."
        exit 1
    fi
}

# Start server with specific thread count
start_server() {
    local threads=$1
    local port=${2:-$DEFAULT_PORT}

    echo "🚀 Starting server (threads: $threads, port: $port)..."
    ./target/release/sk_http_server $port --threads $threads > /dev/null 2>&1 &
    SERVER_PID=$!

    # Wait for server to start
    local max_attempts=10
    local attempt=0
    while [ $attempt -lt $max_attempts ]; do
        if curl -s "http://$SERVER_HOST:$port/health" > /dev/null 2>&1; then
            echo "✅ Server started (PID: $SERVER_PID)"
            return 0
        fi
        sleep 0.5
        attempt=$((attempt + 1))
    done

    echo "❌ Failed to start server"
    return 1
}

# Stop server
stop_server() {
    if [ -n "$SERVER_PID" ]; then
        echo "🛑 Stopping server (PID: $SERVER_PID)..."
        kill $SERVER_PID 2>/dev/null || true
        wait $SERVER_PID 2>/dev/null || true
        SERVER_PID=""
    fi
}

# Warmup server
warmup_server() {
    local port=${1:-$DEFAULT_PORT}
    echo "🔥 Warming up server with $WARMUP_REQUESTS requests..."

    for i in $(seq 1 $WARMUP_REQUESTS); do
        curl -s "http://$SERVER_HOST:$port/eval?expr=2+2" > /dev/null &
        if [ $((i % 10)) -eq 0 ]; then
            wait
        fi
    done
    wait
    echo "✅ Warmup complete"
}

# Test latency and basic performance
test_basic_performance() {
    local port=${1:-$DEFAULT_PORT}
    echo
    echo "📊 Basic Performance Test"
    echo "========================"

    # Test different expression types
    local expressions=(
        "2+2"
        "10*5+3/2"
        "2^10+sqrt(144)"
        ":a:=10;:b:=20;:a*:b"
        "[1,100].sum()"
        "[1,2,3,4,5].map(:x*2).sum()"
    )

    local expr_names=(
        "Simple arithmetic"
        "Mixed operations"
        "Math functions"
        "Variable assignment"
        "Range operations"
        "Array operations"
    )

    for i in "${!expressions[@]}"; do
        local expr="${expressions[$i]}"
        local name="${expr_names[$i]}"

        echo "Testing: $name"

        # Single request timing
        local start_time=$(date +%s%N)
        local response=$(curl -s "http://$SERVER_HOST:$port/eval?expr=$(echo "$expr" | jq -sRr @uri)")
        local end_time=$(date +%s%N)
        local duration=$(((end_time - start_time) / 1000000))

        if echo "$response" | jq -e '.success' > /dev/null 2>&1; then
            local server_time=$(echo "$response" | jq -r '.execution_time_ms')
            echo "  ✅ Success - Client: ${duration}ms, Server: ${server_time}ms"
        else
            echo "  ❌ Failed - Response: $response"
        fi
    done
}

# Test concurrent performance using curl
test_concurrent_performance() {
    local port=${1:-$DEFAULT_PORT}
    local threads=${2:-4}

    echo
    echo "🔄 Concurrent Performance Test (Threads: $threads)"
    echo "================================================="

    for concurrent in "${CONCURRENT_LEVELS[@]}"; do
        echo "Testing with $concurrent concurrent requests..."

        local temp_file="/tmp/skillet_bench_$$"
        local start_time=$(date +%s%N)

        # Launch concurrent requests
        for i in $(seq 1 $concurrent); do
            {
                local req_start=$(date +%s%N)
                local response=$(curl -s "http://$SERVER_HOST:$port/eval?expr=10*5+sqrt(25)")
                local req_end=$(date +%s%N)
                local req_duration=$(((req_end - req_start) / 1000000))

                if echo "$response" | jq -e '.success' > /dev/null 2>&1; then
                    echo "$req_duration" >> "$temp_file"
                else
                    echo "ERROR" >> "$temp_file"
                fi
            } &
        done

        wait
        local end_time=$(date +%s%N)
        local total_duration=$(((end_time - start_time) / 1000000))

        # Calculate statistics
        local success_count=$(grep -v "ERROR" "$temp_file" | wc -l)
        local error_count=$(grep "ERROR" "$temp_file" | wc -l)

        if [ $success_count -gt 0 ]; then
            local avg_latency=$(awk '{sum+=$1} END {printf "%.2f", sum/NR}' "$temp_file")
            local min_latency=$(sort -n "$temp_file" | head -1)
            local max_latency=$(sort -n "$temp_file" | tail -1)
            local throughput=$(echo "scale=2; $success_count * 1000 / $total_duration" | bc)

            echo "  Concurrent: $concurrent, Success: $success_count, Errors: $error_count"
            echo "  Latency - Avg: ${avg_latency}ms, Min: ${min_latency}ms, Max: ${max_latency}ms"
            echo "  Throughput: ${throughput} req/sec"
        else
            echo "  ❌ All requests failed"
        fi

        rm -f "$temp_file"
        echo

        # Brief pause between tests
        sleep 1
    done
}

# Test with Apache Bench if available
test_with_apache_bench() {
    local port=${1:-$DEFAULT_PORT}
    local threads=${2:-4}

    if ! command -v ab &> /dev/null; then
        echo "⚠️  Apache Bench (ab) not available, skipping advanced benchmarks"
        return
    fi

    echo
    echo "🏃 Apache Bench Performance Test (Threads: $threads)"
    echo "=================================================="

    local url="http://$SERVER_HOST:$port/eval?expr=10*5%2Bsqrt(25)"

    for concurrent in 1 10 50 100; do
        local total_requests=$((concurrent * 20))
        echo "Apache Bench: $total_requests requests, $concurrent concurrent..."

        ab -n $total_requests -c $concurrent -q "$url" 2>/dev/null | \
        grep -E "(Requests per second|Time per request|Connection Times)" | \
        sed 's/^/  /'
        echo
    done
}

# Test with wrk if available
test_with_wrk() {
    local port=${1:-$DEFAULT_PORT}
    local threads=${2:-4}

    if ! command -v wrk &> /dev/null; then
        echo "⚠️  wrk not available, skipping wrk benchmarks"
        return
    fi

    echo
    echo "⚡ wrk Performance Test (Threads: $threads)"
    echo "========================================"

    local url="http://$SERVER_HOST:$port/eval?expr=10*5%2Bsqrt(25)"

    echo "wrk: 30s test with 10 concurrent connections..."
    wrk -t10 -c10 -d30s "$url" | sed 's/^/  /'
    echo
}

# Test different thread counts
test_thread_scaling() {
    echo
    echo "🧵 Thread Scaling Performance Test"
    echo "=================================="

    local results_file="/tmp/skillet_thread_results_$$"
    echo "threads,throughput,avg_latency,min_latency,max_latency" > "$results_file"

    for thread_count in "${THREAD_COUNTS[@]}"; do
        local port=$((DEFAULT_PORT + thread_count))

        echo "Testing with $thread_count threads (port $port)..."

        if start_server $thread_count $port; then
            warmup_server $port

            # Run fixed concurrent test
            local concurrent=20
            local temp_file="/tmp/skillet_scaling_$$"
            local start_time=$(date +%s%N)

            for i in $(seq 1 $concurrent); do
                {
                    local req_start=$(date +%s%N)
                    local response=$(curl -s "http://$SERVER_HOST:$port/eval?expr=10*5+sqrt(25)")
                    local req_end=$(date +%s%N)
                    local req_duration=$(((req_end - req_start) / 1000000))

                    if echo "$response" | jq -e '.success' > /dev/null 2>&1; then
                        echo "$req_duration" >> "$temp_file"
                    fi
                } &
            done

            wait
            local end_time=$(date +%s%N)
            local total_duration=$(((end_time - start_time) / 1000000))

            local success_count=$(wc -l < "$temp_file")
            if [ $success_count -gt 0 ]; then
                local avg_latency=$(awk '{sum+=$1} END {printf "%.2f", sum/NR}' "$temp_file")
                local min_latency=$(sort -n "$temp_file" | head -1)
                local max_latency=$(sort -n "$temp_file" | tail -1)
                local throughput=$(echo "scale=2; $success_count * 1000 / $total_duration" | bc)

                echo "  Throughput: ${throughput} req/sec, Avg latency: ${avg_latency}ms"
                echo "$thread_count,$throughput,$avg_latency,$min_latency,$max_latency" >> "$results_file"
            fi

            rm -f "$temp_file"
            stop_server
            sleep 1
        fi
    done

    # Display results summary
    echo
    echo "📈 Thread Scaling Results:"
    echo "Threads | Throughput | Avg Latency | Min | Max"
    echo "--------|------------|-------------|-----|----"
    tail -n +2 "$results_file" | while IFS=, read -r threads throughput avg min max; do
        printf "%7s | %10s | %11s | %3s | %3s\n" "$threads" "$throughput" "${avg}ms" "${min}ms" "${max}ms"
    done

    rm -f "$results_file"
}

# Resource monitoring
monitor_resources() {
    local port=${1:-$DEFAULT_PORT}
    local threads=${2:-4}

    echo
    echo "📊 Resource Usage Test (Threads: $threads)"
    echo "========================================="

    if start_server $threads $port; then
        warmup_server $port

        echo "Monitoring server resource usage during load test..."

        # Start resource monitoring
        local monitor_file="/tmp/skillet_monitor_$$"
        {
            while kill -0 $SERVER_PID 2>/dev/null; do
                if command -v ps &> /dev/null; then
                    ps -p $SERVER_PID -o %cpu,%mem,vsz,rss --no-headers >> "$monitor_file"
                fi
                sleep 0.5
            done
        } &
        local monitor_pid=$!

        # Generate load for 10 seconds
        local end_time=$(($(date +%s) + 10))
        while [ $(date +%s) -lt $end_time ]; do
            for i in {1..10}; do
                curl -s "http://$SERVER_HOST:$port/eval?expr=range(1,50).sum()" > /dev/null &
            done
            wait
        done

        # Stop monitoring and server
        kill $monitor_pid 2>/dev/null || true
        stop_server

        # Analyze resource usage
        if [ -f "$monitor_file" ] && [ -s "$monitor_file" ]; then
            local avg_cpu=$(awk '{sum+=$1} END {printf "%.1f", sum/NR}' "$monitor_file")
            local avg_mem=$(awk '{sum+=$2} END {printf "%.1f", sum/NR}' "$monitor_file")
            local max_rss=$(awk '{if($4>max) max=$4} END {printf "%.0f", max}' "$monitor_file")

            echo "  Average CPU usage: ${avg_cpu}%"
            echo "  Average Memory usage: ${avg_mem}%"
            echo "  Peak RSS memory: ${max_rss} KB"
        else
            echo "  ⚠️  Could not collect resource usage data"
        fi

        rm -f "$monitor_file"
    fi
}

# Main execution
main() {
    echo "Checking dependencies..."
    check_dependencies
    echo "✅ All dependencies available"
    echo

    # Test with default thread count
    local default_threads=4
    local port=$DEFAULT_PORT

    if start_server $default_threads $port; then
        warmup_server $port
        test_basic_performance $port
        test_concurrent_performance $port $default_threads
        test_with_apache_bench $port $default_threads
        test_with_wrk $port $default_threads
        stop_server
    else
        echo "❌ Failed to start server for basic tests"
        exit 1
    fi

    # Test thread scaling
    test_thread_scaling

    # Resource monitoring test
    monitor_resources $((DEFAULT_PORT + 100)) 8

    echo
    echo "🎉 Benchmark Complete!"
    echo "====================="
    echo "Results summary:"
    echo "- Basic performance: Various expression types tested"
    echo "- Concurrent performance: Up to 100 concurrent requests"
    echo "- Thread scaling: 1-32 worker threads tested"
    echo "- Resource usage: CPU and memory monitoring"
    echo
}

# Trap to ensure server cleanup
trap 'stop_server; exit 1' INT TERM EXIT

# Run main function
main "$@"

# Remove trap
trap - INT TERM EXIT