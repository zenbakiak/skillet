#!/bin/bash

# Simple Skillet HTTP Server Benchmark
# Basic performance testing with minimal dependencies (only curl and jq needed)

set -e

echo "=== Skillet HTTP Server Simple Benchmark ==="
echo "Date: $(date)"
echo

# Configuration
SERVER_HOST="127.0.0.1"
BASE_PORT=9000

# Build release version
echo "🔧 Building release version..."
cargo build --release --quiet
echo "✅ Build complete"
echo

# Check basic dependencies
if ! command -v curl &> /dev/null; then
    echo "❌ curl is required but not installed"
    exit 1
fi

if ! command -v jq &> /dev/null; then
    echo "❌ jq is required but not installed"  
    exit 1
fi

# Start server
start_server() {
    local threads=$1
    local port=$2
    
    echo "🚀 Starting server (threads: $threads, port: $port)..."
    ./target/release/sk_http_server $port --threads $threads > /dev/null 2>&1 &
    SERVER_PID=$!
    
    # Wait for server
    local attempts=0
    while [ $attempts -lt 20 ]; do
        if curl -s "http://$SERVER_HOST:$port/health" > /dev/null 2>&1; then
            echo "✅ Server ready (PID: $SERVER_PID)"
            return 0
        fi
        sleep 0.25
        attempts=$((attempts + 1))
    done
    
    echo "❌ Server failed to start"
    return 1
}

# Stop server
stop_server() {
    if [ -n "$SERVER_PID" ]; then
        echo "🛑 Stopping server..."
        kill $SERVER_PID 2>/dev/null || true
        wait $SERVER_PID 2>/dev/null || true
        SERVER_PID=""
        sleep 1
    fi
}

# Test basic performance
test_expressions() {
    local port=$1
    echo
    echo "📊 Expression Performance Test"
    echo "============================="
    
    # Test cases: expression, name, expected_complexity
    local tests=(
        "2+2|Simple arithmetic|low"
        "10*5+3/2-1|Mixed arithmetic|low"  
        "2^10+sqrt(144)*3|Math functions|medium"
        ":a:=10;:b:=20;:c:=:a*:b|Variable assignment|medium"
        "range(1,50).sum()|Range operations|high"
        "[1,2,3,4,5,6,7,8,9,10].map(:x*2).filter(:x>10).sum()|Complex array ops|high"
    )
    
    for test_case in "${tests[@]}"; do
        IFS='|' read -r expr name complexity <<< "$test_case"
        
        echo "Testing: $name ($complexity complexity)"
        
        # Run test 5 times to get average
        local total_client_time=0
        local total_server_time=0
        local success_count=0
        
        for i in {1..5}; do
            local start_time=$(date +%s%N)
            local encoded_expr=$(echo "$expr" | sed 's/+/%2B/g; s/:/%3A/g; s/\[/%5B/g; s/\]/%5D/g; s/,/%2C/g; s/(/%28/g; s/)/%29/g')
            local response=$(curl -s "http://$SERVER_HOST:$port/eval?expr=$encoded_expr")
            local end_time=$(date +%s%N)
            
            local client_time=$(((end_time - start_time) / 1000000))
            
            if echo "$response" | jq -e '.success' > /dev/null 2>&1; then
                local server_time=$(echo "$response" | jq -r '.execution_time_ms')
                total_client_time=$((total_client_time + client_time))
                total_server_time=$(echo "$total_server_time + $server_time" | bc)
                success_count=$((success_count + 1))
            fi
        done
        
        if [ $success_count -eq 5 ]; then
            local avg_client=$((total_client_time / 5))
            local avg_server=$(echo "scale=2; $total_server_time / 5" | bc)
            echo "  ✅ Avg - Client: ${avg_client}ms, Server: ${avg_server}ms"
        else
            echo "  ❌ Failed ($success_count/5 succeeded)"
        fi
        
        sleep 0.1
    done
}

# Test concurrent requests
test_concurrent() {
    local port=$1
    local threads=$2
    echo
    echo "🔄 Concurrent Performance Test (Server threads: $threads)"
    echo "================================================="
    
    local concurrent_levels=(1 5 10 20 50)
    
    for concurrent in "${concurrent_levels[@]}"; do
        echo "Testing $concurrent concurrent requests..."
        
        local temp_file="/tmp/skillet_concurrent_$$"
        local start_time=$(date +%s%N)
        
        # Launch concurrent requests
        for i in $(seq 1 $concurrent); do
            {
                local req_start=$(date +%s%N)
                local response=$(curl -s "http://$SERVER_HOST:$port/eval?expr=10*5%2Bsqrt(25)")
                local req_end=$(date +%s%N)
                local req_time=$(((req_end - req_start) / 1000000))
                
                if echo "$response" | jq -e '.success' > /dev/null 2>&1; then
                    echo "$req_time" >> "$temp_file"
                fi
            } &
        done
        
        wait
        local total_time=$((($(date +%s%N) - start_time) / 1000000))
        
        # Calculate stats
        local success_count=$(wc -l < "$temp_file")
        
        if [ $success_count -gt 0 ]; then
            local avg_latency=$(awk '{sum+=$1} END {printf "%.1f", sum/NR}' "$temp_file")
            local min_latency=$(sort -n "$temp_file" | head -1)
            local max_latency=$(sort -n "$temp_file" | tail -1)
            local throughput=$(echo "scale=1; $success_count * 1000 / $total_time" | bc)
            
            echo "  Success: $success_count/$concurrent"
            echo "  Latency - Avg: ${avg_latency}ms, Min: ${min_latency}ms, Max: ${max_latency}ms"
            echo "  Throughput: ${throughput} req/sec"
        else
            echo "  ❌ All requests failed"
        fi
        
        rm -f "$temp_file"
        echo
    done
}

# Test thread scaling
test_thread_scaling() {
    echo
    echo "🧵 Thread Scaling Test"
    echo "===================="
    
    local thread_counts=(1 2 4 8 16)
    local concurrent=20
    
    echo "Testing with $concurrent concurrent requests per thread count..."
    echo
    printf "%-8s | %-12s | %-11s | %-10s\n" "Threads" "Throughput" "Avg Latency" "Success Rate"
    echo "---------|--------------|-------------|----------"
    
    for thread_count in "${thread_counts[@]}"; do
        local port=$((BASE_PORT + thread_count))
        
        if start_server $thread_count $port; then
            # Warmup
            curl -s "http://$SERVER_HOST:$port/eval?expr=2%2B2" > /dev/null
            
            # Test
            local temp_file="/tmp/skillet_scaling_$$"
            local start_time=$(date +%s%N)
            
            for i in $(seq 1 $concurrent); do
                {
                    local req_start=$(date +%s%N)
                    local response=$(curl -s "http://$SERVER_HOST:$port/eval?expr=range(1,25).sum()")
                    local req_end=$(date +%s%N)
                    local req_time=$(((req_end - req_start) / 1000000))
                    
                    if echo "$response" | jq -e '.success' > /dev/null 2>&1; then
                        echo "$req_time" >> "$temp_file"
                    fi
                } &
            done
            
            wait
            local total_time=$((($(date +%s%N) - start_time) / 1000000))
            
            local success_count=$(wc -l < "$temp_file" 2>/dev/null || echo "0")
            
            if [ $success_count -gt 0 ]; then
                local avg_latency=$(awk '{sum+=$1} END {printf "%.1f", sum/NR}' "$temp_file")
                local throughput=$(echo "scale=1; $success_count * 1000 / $total_time" | bc)
                local success_rate=$(echo "scale=1; $success_count * 100 / $concurrent" | bc)
                
                printf "%-8s | %-12s | %-11s | %-10s\n" \
                    "$thread_count" "${throughput} req/s" "${avg_latency}ms" "${success_rate}%"
            else
                printf "%-8s | %-12s | %-11s | %-10s\n" \
                    "$thread_count" "Failed" "N/A" "0%"
            fi
            
            rm -f "$temp_file"
            stop_server
        fi
    done
}

# Test variable inclusion performance
test_variable_performance() {
    local port=$1
    echo
    echo "📋 Variable Inclusion Performance"
    echo "================================"
    
    local expression=":a:=10;:b:=20;:c:=30;:d:=40;:result:=:a*:b+:c*:d"
    local encoded_expr=$(echo "$expression" | sed 's/+/%2B/g; s/:/%3A/g; s/\*/%2A/g; s/;/%3B/g; s/=/%3D/g')
    
    echo "Expression: $expression"
    echo
    
    # Test different variable inclusion scenarios
    local scenarios=(
        "false|No variables"
        "true|All variables"
        ":a,:result|Selected variables"
    )
    
    for scenario in "${scenarios[@]}"; do
        IFS='|' read -r include_vars description <<< "$scenario"
        
        echo "Testing: $description (include_variables: $include_vars)"
        
        local total_time=0
        local total_size=0
        local success_count=0
        
        for i in {1..10}; do
            local start_time=$(date +%s%N)
            
            local response
            if [ "$include_vars" = "false" ] || [ "$include_vars" = "true" ]; then
                response=$(curl -s -X POST "http://$SERVER_HOST:$port/eval" \
                    -H "Content-Type: application/json" \
                    -d "{\"expression\":\"$expression\",\"include_variables\":$include_vars}")
            else
                response=$(curl -s -X POST "http://$SERVER_HOST:$port/eval" \
                    -H "Content-Type: application/json" \
                    -d "{\"expression\":\"$expression\",\"include_variables\":\"$include_vars\"}")
            fi
            
            local end_time=$(date +%s%N)
            local req_time=$(((end_time - start_time) / 1000000))
            
            if echo "$response" | jq -e '.success' > /dev/null 2>&1; then
                total_time=$((total_time + req_time))
                total_size=$((total_size + $(echo "$response" | wc -c)))
                success_count=$((success_count + 1))
            fi
        done
        
        if [ $success_count -gt 0 ]; then
            local avg_time=$((total_time / success_count))
            local avg_size=$((total_size / success_count))
            echo "  ✅ Avg time: ${avg_time}ms, Avg response size: ${avg_size} bytes"
        else
            echo "  ❌ All requests failed"
        fi
        
        sleep 0.1
    done
}

# Main execution
main() {
    local test_port=$BASE_PORT
    local test_threads=4
    
    echo "🧪 Starting comprehensive HTTP server benchmark..."
    echo
    
    # Single server tests
    if start_server $test_threads $test_port; then
        test_expressions $test_port
        test_concurrent $test_port $test_threads
        test_variable_performance $test_port
        stop_server
    else
        echo "❌ Failed to start server for basic tests"
        exit 1
    fi
    
    # Thread scaling test
    test_thread_scaling
    
    echo
    echo "🎉 Benchmark Complete!"
    echo "====================="
    echo "Key findings:"
    echo "- Expression performance varies by complexity"
    echo "- Thread pool handles concurrent requests efficiently"
    echo "- Optimal thread count depends on workload"
    echo "- Variable selection reduces response size"
    echo
}

# Cleanup on exit
trap 'stop_server; exit 1' INT TERM EXIT

# Run benchmark
main "$@"

# Remove cleanup trap
trap - INT TERM EXIT