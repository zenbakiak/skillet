#!/bin/bash

# Interactive Skillet Server Demo
set -e

echo "🚀 Skillet High-Performance Server Demo"
echo "======================================"

# Build if needed
if [ ! -f "target/release/sk_server" ]; then
    echo "Building server binary..."
    cargo build --release --bin sk_server --bin sk_client
fi

PORT=8888
echo "Starting Skillet server on port $PORT..."
./target/release/sk_server $PORT 4 > server.log 2>&1 &
SERVER_PID=$!

# Cleanup on exit
cleanup() {
    echo ""
    echo "Stopping server..."
    if kill -0 $SERVER_PID 2>/dev/null; then
        kill $SERVER_PID
        wait $SERVER_PID 2>/dev/null || true
    fi
    echo "Server stopped."
}
trap cleanup EXIT

# Wait for server to start
sleep 2

echo "Server started! (PID: $SERVER_PID)"
echo ""
echo "🧮 Let's try some calculations..."
echo ""

# Demo 1: Basic arithmetic
echo "1. Basic Arithmetic:"
echo "   Expression: =2 + 3 * 4"
result=$(./target/release/sk_client localhost:$PORT "=2 + 3 * 4" 2>/dev/null)
echo "   Result: $result"
echo ""

# Demo 2: Functions
echo "2. Built-in Functions:"
echo "   Expression: =SUM(1, 2, 3, 4, 5)"
result=$(./target/release/sk_client localhost:$PORT "=SUM(1, 2, 3, 4, 5)" 2>/dev/null)
echo "   Result: $result"
echo ""

# Demo 3: Variables
echo "3. Using Variables:"
echo "   Expression: =:price * :quantity"
echo "   Variables: price=19.99, quantity=3"
result=$(./target/release/sk_client localhost:$PORT "=:price * :quantity" price=19.99 quantity=3 2>/dev/null)
echo "   Result: $result"
echo ""

# Demo 4: JSON variables
echo "4. JSON Variables:"
echo "   Expression: =:user.name"
echo "   JSON: {\"user\": {\"name\": \"Alice\", \"age\": 30}}"
result=$(./target/release/sk_client localhost:$PORT "=:user.name" --json '{"user": {"name": "Alice", "age": 30}}' 2>/dev/null)
echo "   Result: $result"
echo ""

# Demo 5: Complex expression
echo "5. Complex Expression:"
echo "   Expression: =MAX(SUM(1,2,3), AVG(10,20,30), MIN(100,200))"
result=$(./target/release/sk_client localhost:$PORT "=MAX(SUM(1,2,3), AVG(10,20,30), MIN(100,200))" 2>/dev/null)
echo "   Result: $result"
echo ""

# Demo 6: Performance test
echo "6. Performance Test (100 operations):"
echo "   Testing throughput with =2*3+4*5..."
echo ""
./target/release/sk_client localhost:$PORT --benchmark "=2*3+4*5" 100
echo ""

# Demo 7: Direct protocol test
echo "7. Direct Protocol Test:"
echo "   Sending JSON directly to server..."
echo "   Request: {\"expression\": \"=42\", \"variables\": null}"
response=$(echo '{"expression": "=42", "variables": null}' | nc localhost $PORT 2>/dev/null)
echo "   Response: $response"
echo ""

# Interactive mode
echo "🎮 Interactive Mode"
echo "==================="
echo "You can now test expressions interactively!"
echo "Examples:"
echo "  ./target/release/sk_client localhost:$PORT \"=2+3*4\""
echo "  ./target/release/sk_client localhost:$PORT \"=SUM(:a, :b)\" a=10 b=20"
echo "  ./target/release/sk_client localhost:$PORT --benchmark \"=2+3\" 50"
echo ""
echo "Or test with raw JSON:"
echo "  echo '{\"expression\": \"=123\", \"variables\": null}' | nc localhost $PORT"
echo ""
echo "🔍 Server details:"
echo "  • Port: $PORT"
echo "  • PID: $SERVER_PID"
echo "  • Logs: server.log"
echo "  • Workers: 4 threads"
echo ""
echo "Press Ctrl+C to stop the server and exit."
echo ""

# Keep the script running so server stays up
echo "Server running... (Press Ctrl+C to stop)"
while true; do
    if ! kill -0 $SERVER_PID 2>/dev/null; then
        echo "Server has stopped unexpectedly!"
        cat server.log
        exit 1
    fi
    sleep 5
done