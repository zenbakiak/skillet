#!/bin/bash

# Performance Testing Suite for Skillet
# This script runs a comprehensive performance and concurrency test suite

set -e

echo "🔧 Skillet Performance Testing Suite"
echo "====================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "src" ]; then
    print_error "Please run this script from the skillet project root directory"
    exit 1
fi

# Build release version for accurate performance testing
print_status "Building release version..."
if cargo build --release --bin sk; then
    print_success "Release build completed"
else
    print_error "Failed to build release version"
    exit 1
fi

# Function to run a test and capture results
run_test() {
    local test_name="$1"
    local test_command="$2"
    local log_file="performance_${test_name}.log"
    
    print_status "Running $test_name..."
    echo "Command: $test_command" > "$log_file"
    echo "Started at: $(date)" >> "$log_file"
    echo "----------------------------------------" >> "$log_file"
    
    if eval "$test_command" >> "$log_file" 2>&1; then
        print_success "$test_name completed successfully"
        return 0
    else
        print_error "$test_name failed - check $log_file for details"
        return 1
    fi
}

# Create results directory
mkdir -p performance_results
cd performance_results

print_status "Starting performance test suite..."
echo "Results will be saved in: $(pwd)"
echo ""

# Track overall results
total_tests=0
passed_tests=0

# Run performance benchmarks
print_status "=== PERFORMANCE BENCHMARKS ==="

tests=(
    "arithmetic_ops:cargo test --test sk_performance test_benchmark_arithmetic_operations --release"
    "function_ops:cargo test --test sk_performance test_benchmark_function_operations --release"
    "json_ops:cargo test --test sk_performance test_benchmark_json_operations --release"  
    "memory_usage:cargo test --test sk_performance test_memory_usage_test --release"
)

for test in "${tests[@]}"; do
    IFS=':' read -r test_name test_cmd <<< "$test"
    total_tests=$((total_tests + 1))
    if run_test "$test_name" "$test_cmd"; then
        passed_tests=$((passed_tests + 1))
    fi
done

echo ""
print_status "=== CONCURRENCY TESTS ==="

concurrency_tests=(
    "basic_concurrent:cargo test --test sk_concurrency test_concurrent_basic_operations --release"
    "variable_concurrent:cargo test --test sk_concurrency test_concurrent_variable_operations --release"
    "json_concurrent:cargo test --test sk_concurrency test_concurrent_json_operations --release"
    "stress_test:cargo test --test sk_concurrency test_stress_test_high_concurrency --release"
    "resource_cleanup:cargo test --test sk_concurrency test_resource_cleanup --release"
)

for test in "${concurrency_tests[@]}"; do
    IFS=':' read -r test_name test_cmd <<< "$test"
    total_tests=$((total_tests + 1))
    if run_test "$test_name" "$test_cmd"; then
        passed_tests=$((passed_tests + 1))
    fi
done

# Quick smoke tests to verify basic functionality
echo ""
print_status "=== SMOKE TESTS ==="

smoke_tests=(
    "basic_arithmetic" "cargo run --release --bin sk -- '=2 + 3 * 4'"
    "variable_test" "cargo run --release --bin sk -- '=:x + :y' x=10 y=20"
    "json_test" "cargo run --release --bin sk -- '=:user.name' --json '{\"user\": {\"name\": \"test\"}}'"
    "function_test" "cargo run --release --bin sk -- '=SUM(1, 2, 3, 4, 5)'"
)

smoke_passed=0
smoke_total=0

for ((i=0; i<${#smoke_tests[@]}; i+=2)); do
    test_name="${smoke_tests[i]}"
    test_cmd="${smoke_tests[i+1]}"
    smoke_total=$((smoke_total + 1))
    
    print_status "Running smoke test: $test_name"
    if eval "$test_cmd" > "smoke_${test_name}.log" 2>&1; then
        print_success "Smoke test $test_name passed"
        smoke_passed=$((smoke_passed + 1))
    else
        print_warning "Smoke test $test_name failed - check smoke_${test_name}.log"
    fi
done

# System information
echo ""
print_status "=== SYSTEM INFORMATION ==="
{
    echo "Test run completed at: $(date)"
    echo "System: $(uname -a)"
    echo "Rust version: $(rustc --version)"
    echo "Cargo version: $(cargo --version)"
    echo "CPU info:"
    if command -v lscpu >/dev/null 2>&1; then
        lscpu | grep -E "Model name|CPU\(s\)|Thread"
    elif [ -f /proc/cpuinfo ]; then
        grep -E "model name|cpu cores|siblings" /proc/cpuinfo | head -3
    elif command -v sysctl >/dev/null 2>&1; then
        sysctl -n hw.model hw.ncpu
    fi
    echo "Memory info:"
    if command -v free >/dev/null 2>&1; then
        free -h
    elif command -v vm_stat >/dev/null 2>&1; then
        vm_stat
    fi
} > system_info.log

# Generate summary report
{
    echo "SKILLET PERFORMANCE TEST SUMMARY"
    echo "==============================="
    echo "Date: $(date)"
    echo "Total tests: $total_tests"
    echo "Passed tests: $passed_tests"
    echo "Failed tests: $((total_tests - passed_tests))"
    echo "Success rate: $(echo "scale=2; $passed_tests * 100 / $total_tests" | bc -l)%"
    echo ""
    echo "Smoke tests: $smoke_passed/$smoke_total passed"
    echo ""
    echo "Log files generated:"
    ls -la *.log
    echo ""
    echo "For detailed results, check individual log files."
} > test_summary.txt

# Display final results
echo ""
echo "==============================="
print_status "TEST SUITE COMPLETED"
echo "==============================="
cat test_summary.txt

if [ $passed_tests -eq $total_tests ] && [ $smoke_passed -eq $smoke_total ]; then
    print_success "All tests passed! 🎉"
    exit 0
elif [ $passed_tests -gt 0 ]; then
    print_warning "Some tests failed. Check log files for details."
    exit 1
else
    print_error "All tests failed. There may be a serious issue."
    exit 2
fi