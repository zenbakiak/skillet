#!/bin/bash
#
# Skillet HTTP Server Daemon Management Script
#

BINARY="sk_http_server"
PID_FILE="skillet-http.pid"
PORT="${SKILLET_PORT:-5074}"
HOST="${SKILLET_HOST:-0.0.0.0}"
TOKEN="${SKILLET_AUTH_TOKEN}"

usage() {
    echo "Usage: $0 {start|stop|restart|status} [options]"
    echo ""
    echo "Commands:"
    echo "  start     Start the daemon"
    echo "  stop      Stop the daemon"
    echo "  restart   Restart the daemon"
    echo "  status    Show daemon status"
    echo ""
    echo "Environment variables:"
    echo "  SKILLET_PORT=5074           Server port (default: 5074)"
    echo "  SKILLET_HOST=0.0.0.0        Server host (default: 0.0.0.0)"
    echo "  SKILLET_AUTH_TOKEN=token    Authentication token (optional)"
    echo ""
    echo "Examples:"
    echo "  $0 start"
    echo "  SKILLET_PORT=8080 $0 start"
    echo "  SKILLET_AUTH_TOKEN=secret123 $0 start"
    exit 1
}

start_daemon() {
    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE" 2>/dev/null)
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            echo "Skillet HTTP server is already running (PID: $pid)"
            exit 1
        else
            echo "Removing stale PID file..."
            rm -f "$PID_FILE"
        fi
    fi

    # Build command
    local cmd="$BINARY $PORT --host $HOST -d --pid-file $PID_FILE"
    if [ -n "$TOKEN" ]; then
        cmd="$cmd --token $TOKEN"
    fi

    echo "Starting Skillet HTTP server..."
    echo "Port: $PORT, Host: $HOST, PID file: $PID_FILE"
    if [ -n "$TOKEN" ]; then
        echo "Token auth: enabled"
    fi

    # Check if binary exists
    if [ ! -f "$BINARY" ]; then
        echo "Error: Binary not found at $BINARY"
        echo "Run: cargo build --release --bin sk_http_server"
        exit 1
    fi

    # Start daemon
    $cmd

    # Wait a moment and check if it started
    sleep 1
    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            echo "✅ Skillet HTTP server started successfully (PID: $pid)"
            echo "🌐 Server running at http://$HOST:$PORT"
        else
            echo "❌ Failed to start server"
            exit 1
        fi
    else
        echo "❌ PID file not created, startup may have failed"
        exit 1
    fi
}

stop_daemon() {
    if [ ! -f "$PID_FILE" ]; then
        echo "PID file not found. Server may not be running."
        exit 1
    fi

    local pid=$(cat "$PID_FILE" 2>/dev/null)
    if [ -z "$pid" ]; then
        echo "Invalid PID file"
        rm -f "$PID_FILE"
        exit 1
    fi

    if ! kill -0 "$pid" 2>/dev/null; then
        echo "Process $pid not found. Removing stale PID file."
        rm -f "$PID_FILE"
        exit 1
    fi

    echo "Stopping Skillet HTTP server (PID: $pid)..."
    kill "$pid"

    # Wait for graceful shutdown
    for i in {1..10}; do
        if ! kill -0 "$pid" 2>/dev/null; then
            rm -f "$PID_FILE"
            echo "✅ Server stopped successfully"
            exit 0
        fi
        sleep 1
    done

    # Force kill if still running
    echo "Force killing server..."
    kill -9 "$pid" 2>/dev/null
    rm -f "$PID_FILE"
    echo "✅ Server force-stopped"
}

show_status() {
    if [ ! -f "$PID_FILE" ]; then
        echo "❌ Skillet HTTP server is not running (no PID file)"
        exit 1
    fi

    local pid=$(cat "$PID_FILE" 2>/dev/null)
    if [ -z "$pid" ]; then
        echo "❌ Invalid PID file"
        exit 1
    fi

    if kill -0 "$pid" 2>/dev/null; then
        echo "✅ Skillet HTTP server is running (PID: $pid)"
        echo "🌐 Server: http://$HOST:$PORT"

        # Try to get health status
        if command -v curl >/dev/null 2>&1; then
            echo ""
            if [ -n "$TOKEN" ]; then
                echo "Health check (with auth):"
                curl -s -H "Authorization: Bearer $TOKEN" "http://$HOST:$PORT/health" | jq . 2>/dev/null || echo "Health check failed or jq not available"
            else
                echo "Health check:"
                curl -s "http://$HOST:$PORT/health" | jq . 2>/dev/null || echo "Health check failed or jq not available"
            fi
        fi
    else
        echo "❌ PID file exists but process $pid is not running"
        rm -f "$PID_FILE"
        exit 1
    fi
}

case "$1" in
    start)
        start_daemon
        ;;
    stop)
        stop_daemon
        ;;
    restart)
        stop_daemon 2>/dev/null || true
        sleep 1
        start_daemon
        ;;
    status)
        show_status
        ;;
    *)
        usage
        ;;
esac