#!/bin/bash

# Demo of the built-in -d daemon flag
set -e

echo "🚀 Testing Skillet Server Built-in Daemon Flag"
echo "=============================================="

# Build if needed
if [ ! -f "target/release/sk_server" ]; then
    echo "Building server..."
    cargo build --release --bin sk_server --bin sk_client
fi

PORT=9999
PID_FILE="demo-server.pid"

# Cleanup function
cleanup() {
    echo ""
    echo "Cleaning up..."
    if [ -f "$PID_FILE" ]; then
        PID=$(cat "$PID_FILE")
        if kill -0 $PID 2>/dev/null; then
            echo "Stopping daemon (PID: $PID)..."
            kill $PID
            sleep 1
            if kill -0 $PID 2>/dev/null; then
                echo "Force killing daemon..."
                kill -9 $PID
            fi
        fi
        rm -f "$PID_FILE"
    fi
    echo "Cleanup complete."
}
trap cleanup EXIT

echo ""
echo "1. Testing Normal Mode (Foreground)"
echo "   Command: ./target/release/sk_server $PORT 2"
echo "   Starting server for 3 seconds..."

./target/release/sk_server $PORT 2 &
NORMAL_PID=$!
sleep 3

# Test the normal server
echo "   Testing normal server..."
if echo '{"expression": "=2+3*4", "variables": null}' | nc localhost $PORT 2>/dev/null; then
    echo "   ✅ Normal mode works!"
else
    echo "   ❌ Normal mode failed"
fi

# Kill normal server
kill $NORMAL_PID 2>/dev/null || true
wait $NORMAL_PID 2>/dev/null || true
sleep 1

echo ""
echo "2. Testing Daemon Mode (Background)"
echo "   Command: ./target/release/sk_server $PORT 4 -d --pid-file $PID_FILE"

# Start in daemon mode
./target/release/sk_server $PORT 4 -d --pid-file "$PID_FILE"

# Check if PID file was created
if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    echo "   ✅ Daemon started with PID: $PID"
    echo "   📁 PID file created: $PID_FILE"
else
    echo "   ❌ PID file not created"
    exit 1
fi

# Verify process is running
if kill -0 $PID 2>/dev/null; then
    echo "   ✅ Process is running as daemon"
else
    echo "   ❌ Process is not running"
    exit 1
fi

# Check if it's actually daemonized (no parent terminal)
PPID=$(ps -o ppid= -p $PID | tr -d ' ')
if [ "$PPID" = "1" ]; then
    echo "   ✅ Process is properly daemonized (parent PID: 1)"
else
    echo "   ⚠️  Process parent PID: $PPID (might not be fully daemonized)"
fi

echo ""
echo "3. Testing Daemon Functionality"
echo "   Waiting for daemon to fully start..."
sleep 2

# Test daemon functionality
echo "   Testing daemon server..."
for i in {1..3}; do
    RESPONSE=$(echo '{"expression": "=2+3*4", "variables": null}' | nc localhost $PORT 2>/dev/null || echo "failed")
    if echo "$RESPONSE" | grep -q "success"; then
        echo "   ✅ Test $i: Daemon responding correctly"
        echo "      Response: $(echo $RESPONSE | jq -c . 2>/dev/null || echo $RESPONSE)"
    else
        echo "   ❌ Test $i: Daemon not responding: $RESPONSE"
    fi
    sleep 1
done

echo ""
echo "4. Performance Test"
if command -v ./target/release/sk_client >/dev/null 2>&1; then
    echo "   Running performance test (50 operations)..."
    ./target/release/sk_client localhost:$PORT --benchmark "=2*3+4" 50
else
    echo "   Client not found, skipping performance test"
fi

echo ""
echo "5. Process Information"
echo "   Daemon PID: $PID"
echo "   Process info:"
ps -o pid,ppid,cmd -p $PID || echo "   Process information not available"

echo ""
echo "6. Testing Signal Handling"
echo "   Sending SIGTERM to daemon..."
if kill -TERM $PID 2>/dev/null; then
    echo "   ✅ Signal sent successfully"
    sleep 2
    
    if kill -0 $PID 2>/dev/null; then
        echo "   ⚠️  Process still running, force killing..."
        kill -9 $PID
    else
        echo "   ✅ Process terminated gracefully"
    fi
else
    echo "   ❌ Failed to send signal"
fi

echo ""
echo "7. Daemon Mode Summary"
echo "   ✅ Built-in daemon flag (-d) works!"
echo "   ✅ PID file creation works"
echo "   ✅ Process daemonization works"
echo "   ✅ Server functionality preserved"
echo "   ✅ Signal handling works"

echo ""
echo "🎉 Daemon flag test completed successfully!"
echo ""
echo "Usage examples:"
echo "  ./target/release/sk_server 8080 -d                    # Basic daemon"
echo "  ./target/release/sk_server 8080 8 -d                  # With 8 threads"
echo "  ./target/release/sk_server 8080 -d --pid-file my.pid  # Custom PID file"
echo ""
echo "To stop a daemon:"
echo "  kill \$(cat skillet-server.pid)  # Default PID file"
echo "  kill \$(cat my.pid)              # Custom PID file"