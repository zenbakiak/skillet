#!/bin/bash

# Debug daemon startup
set -e

echo "🔍 Debugging Skillet Server Daemon Startup"
echo "==========================================="

# Build if needed
if [ ! -f "target/release/sk_server" ]; then
    echo "Building server..."
    cargo build --release --bin sk_server
fi

PORT=8888
PID_FILE="debug-daemon.pid"

# Cleanup
cleanup() {
    echo ""
    echo "Cleaning up..."
    if [ -f "$PID_FILE" ]; then
        PID=$(cat "$PID_FILE" 2>/dev/null || echo "")
        if [ -n "$PID" ] && kill -0 $PID 2>/dev/null; then
            echo "Killing daemon (PID: $PID)..."
            kill $PID 2>/dev/null || true
            sleep 1
            kill -9 $PID 2>/dev/null || true
        fi
        rm -f "$PID_FILE"
    fi
}
trap cleanup EXIT

echo ""
echo "Step 1: Check if port is available"
if lsof -i :$PORT >/dev/null 2>&1; then
    echo "❌ Port $PORT is already in use:"
    lsof -i :$PORT
    echo "Choose a different port or kill the process using that port"
    exit 1
else
    echo "✅ Port $PORT is available"
fi

echo ""
echo "Step 2: Test server in foreground first"
echo "Starting server in foreground for 3 seconds..."
timeout 3 ./target/release/sk_server $PORT 2 &
FG_PID=$!

# Wait a moment for server to start
sleep 1

# Test foreground server
if echo '{"expression": "=42", "variables": null}' | nc localhost $PORT >/dev/null 2>&1; then
    echo "✅ Foreground server works correctly"
else
    echo "❌ Foreground server not responding"
    kill $FG_PID 2>/dev/null || true
    echo "   Try running manually: ./target/release/sk_server $PORT 2"
    exit 1
fi

# Clean up foreground server
kill $FG_PID 2>/dev/null || true
wait $FG_PID 2>/dev/null || true
sleep 1

echo ""
echo "Step 3: Test daemon mode with verbose output"
echo "Command: ./target/release/sk_server $PORT 4 -d --pid-file $PID_FILE"
echo ""

# Start daemon with output before it redirects
echo "Starting daemon..."
./target/release/sk_server $PORT 4 -d --pid-file "$PID_FILE" 

echo "Daemon command completed."

# Check if PID file exists
echo ""
echo "Step 4: Check daemon status"
if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    echo "✅ PID file created: $PID_FILE"
    echo "   PID: $PID"
    
    # Check if process exists
    if kill -0 $PID 2>/dev/null; then
        echo "✅ Process is running"
        
        # Check process details
        echo "   Process info:"
        ps -o pid,ppid,state,cmd -p $PID || echo "   Unable to get process info"
        
    else
        echo "❌ Process is not running (PID file exists but process doesn't)"
        echo "   This means the daemon started but then exited"
        cat "$PID_FILE"
        rm -f "$PID_FILE"
        exit 1
    fi
else
    echo "❌ PID file not created: $PID_FILE"
    echo "   This means daemonization failed"
    exit 1
fi

echo ""
echo "Step 5: Test daemon functionality"
echo "Waiting 2 seconds for daemon to be ready..."
sleep 2

# Test daemon
for i in {1..3}; do
    echo "Test $i:"
    if RESPONSE=$(echo '{"expression": "=2*3+4", "variables": null}' | timeout 3 nc localhost $PORT 2>&1); then
        if echo "$RESPONSE" | grep -q "success"; then
            echo "   ✅ Daemon responding: $(echo "$RESPONSE" | head -1)"
        else
            echo "   ⚠️  Daemon responded but with unexpected format: $RESPONSE"
        fi
    else
        echo "   ❌ Daemon not responding: $RESPONSE"
        echo "   Checking if process still exists..."
        if kill -0 $PID 2>/dev/null; then
            echo "   Process still running, might be a network issue"
        else
            echo "   Process has died!"
            break
        fi
    fi
done

echo ""
echo "Step 6: Final status check"
if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if kill -0 $PID 2>/dev/null; then
        echo "✅ Daemon is running successfully!"
        echo "   PID: $PID"
        echo "   Port: $PORT"
        echo "   PID file: $PID_FILE"
        echo ""
        echo "To test: echo '{\"expression\": \"=2+3\", \"variables\": null}' | nc localhost $PORT"
        echo "To stop: kill $PID"
    else
        echo "❌ Daemon has stopped running"
    fi
else
    echo "❌ No PID file found"
fi