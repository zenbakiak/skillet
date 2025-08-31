#!/bin/bash

# Simple background daemon for Skillet server
# Alternative to the built-in -d flag for troubleshooting

usage() {
    echo "Usage: $0 <port> [threads] [options]"
    echo ""
    echo "Options:"
    echo "  --pid-file <file>    PID file location (default: skillet-server.pid)"
    echo "  --log-file <file>    Log file location (default: skillet-server.log)"
    echo ""
    echo "Examples:"
    echo "  $0 8080"
    echo "  $0 8080 8 --pid-file /tmp/skillet.pid"
    echo "  $0 8080 4 --log-file /var/log/skillet.log"
    exit 1
}

if [ $# -lt 1 ]; then
    usage
fi

PORT=$1
THREADS=${2:-4}
PID_FILE="skillet-server.pid"
LOG_FILE="skillet-server.log"
shift 2 2>/dev/null || shift 1

# Parse remaining arguments
while [ $# -gt 0 ]; do
    case "$1" in
        --pid-file)
            if [ -n "$2" ]; then
                PID_FILE="$2"
                shift 2
            else
                echo "Error: --pid-file requires a filename"
                exit 1
            fi
            ;;
        --log-file)
            if [ -n "$2" ]; then
                LOG_FILE="$2"
                shift 2
            else
                echo "Error: --log-file requires a filename"
                exit 1
            fi
            ;;
        *)
            echo "Unknown argument: $1"
            usage
            ;;
    esac
done

# Check if server binary exists
if [ ! -f "target/release/sk_server" ]; then
    echo "Error: Server binary not found. Build with: cargo build --release --bin sk_server"
    exit 1
fi

# Check if already running
if [ -f "$PID_FILE" ]; then
    OLD_PID=$(cat "$PID_FILE" 2>/dev/null)
    if [ -n "$OLD_PID" ] && kill -0 "$OLD_PID" 2>/dev/null; then
        echo "Error: Server already running with PID $OLD_PID"
        echo "Stop it first with: kill $OLD_PID"
        exit 1
    else
        echo "Removing stale PID file..."
        rm -f "$PID_FILE"
    fi
fi

echo "Starting Skillet server as background process..."
echo "Port: $PORT"
echo "Threads: $THREADS"  
echo "PID file: $PID_FILE"
echo "Log file: $LOG_FILE"

# Start server in background
nohup ./target/release/sk_server "$PORT" "$THREADS" > "$LOG_FILE" 2>&1 &
SERVER_PID=$!

# Write PID file
echo "$SERVER_PID" > "$PID_FILE"

# Wait a moment to see if it starts successfully
sleep 2

if kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "✅ Server started successfully!"
    echo "   PID: $SERVER_PID"
    echo "   Logs: $LOG_FILE"
    echo "   Control:"
    echo "     Status: kill -0 $SERVER_PID && echo 'Running' || echo 'Stopped'"
    echo "     Stop:   kill $SERVER_PID"
    echo "     Logs:   tail -f $LOG_FILE"
    echo ""
    echo "Test server:"
    echo "  echo '{\"expression\": \"=2+3*4\", \"variables\": null}' | nc localhost $PORT"
else
    echo "❌ Server failed to start!"
    echo "Check logs: cat $LOG_FILE"
    rm -f "$PID_FILE"
    exit 1
fi