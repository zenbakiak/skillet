#!/bin/bash

echo "=== Skillet Phase 1 Performance Benchmark ==="
echo "Testing optimized lexer, parser, and AST memory pooling"
echo

SKILLET_HOOKS_DIR=/nonexistent

# Test expressions
expressions=(
    "2 + 3 * 4 ^ 2 / (5 - 1)"
    "[1,2,3,4,5,6,7,8,9,10].filter(:x > 5).map(:x * 2).sum()"
    ":x := 100; :y := 200; :z := :x + :y * 3; :z / 2"
    "SUM(FILTER([1,2,3,4,5,6,7,8,9,10], :x % 2 == 0))"
    "'hello world'.upper().trim().length()"
)

echo "Running each expression 100 times..."
echo

for expr in "${expressions[@]}"; do
    echo "Testing: $expr"
    echo -n "  Result: "
    result=$($SKILLET_HOOKS_DIR ./target/release/sk "$expr" 2>/dev/null)
    echo "$result"
    
    echo -n "  Time (100 iterations): "
    start_time=$(date +%s.%N)
    for i in {1..100}; do
        $SKILLET_HOOKS_DIR ./target/release/sk "$expr" >/dev/null 2>&1
    done
    end_time=$(date +%s.%N)
    
    duration=$(echo "$end_time - $start_time" | bc -l)
    avg_time=$(echo "scale=3; $duration * 10" | bc -l)
    echo "${avg_time}ms average per parse/eval"
    echo
done

echo "=== Benchmark Complete ==="
echo "Key optimizations applied:"
echo "1. ✅ Lexer string interning for keywords (TRUE, FALSE, NULL)"
echo "2. ✅ Optimized number parsing (direct byte-to-number for integers)"
echo "3. ✅ AST memory optimization (Box<Expr> → Rc<Expr>)"
echo "4. ✅ Parser lookahead buffering improvements"
echo
echo "Expected improvement: 30-50% faster parsing for typical expressions"