# Built-in Daemon Flag (-d) Guide

The Skillet server now has **built-in daemon support** with the `-d` flag! No external scripts needed.

## ⚡ Quick Start

```bash
# Start as daemon (background process)
./target/release/sk_server 8080 -d

# Check it's running
echo '{"expression": "=2+3*4", "variables": null}' | nc localhost 8080

# Stop daemon
kill $(cat skillet-server.pid)
```

## 🎯 All Usage Options

### Basic Usage
```bash
# Foreground (normal)
./target/release/sk_server 8080

# Background (daemon)
./target/release/sk_server 8080 -d
```

### With Thread Count
```bash
# 8 threads in daemon mode
./target/release/sk_server 8080 8 -d

# Flexible argument order
./target/release/sk_server 8080 -d 8
```

### Custom PID File
```bash
# Custom PID file location
./target/release/sk_server 8080 -d --pid-file /var/run/skillet.pid

# Check process
kill -0 $(cat /var/run/skillet.pid) && echo "Running" || echo "Not running"
```

### Advanced Options
```bash
# Full example with all options
./target/release/sk_server 8080 16 -d --pid-file /tmp/skillet.pid --log-file /tmp/skillet.log
```

## 🛠️ Management Commands

### Start Daemon
```bash
# Default PID file (skillet-server.pid)
./target/release/sk_server 8080 -d

# Custom PID file
./target/release/sk_server 8080 -d --pid-file my-server.pid
```

### Check Status
```bash
# Check if running (default PID file)
kill -0 $(cat skillet-server.pid) 2>/dev/null && echo "Running" || echo "Stopped"

# Check if running (custom PID file)
kill -0 $(cat my-server.pid) 2>/dev/null && echo "Running" || echo "Stopped"

# Get process info
ps -p $(cat skillet-server.pid)
```

### Stop Daemon
```bash
# Graceful stop (default PID file)
kill $(cat skillet-server.pid)

# Graceful stop (custom PID file) 
kill $(cat my-server.pid)

# Force stop if needed
kill -9 $(cat skillet-server.pid)
```

### Test Daemon
```bash
# Simple test
echo '{"expression": "=1+1", "variables": null}' | nc localhost 8080

# Performance test
./target/release/sk_client localhost:8080 --benchmark "=2+3*4" 1000
```

## 🔧 How It Works

When you use `-d` flag:

1. **Double Fork**: Creates proper daemon process
2. **Session Leader**: Detaches from controlling terminal
3. **File Descriptors**: Redirects stdin/stdout/stderr to `/dev/null`
4. **Working Directory**: Changes to root (`/`)
5. **PID File**: Writes process ID to file
6. **Signal Handling**: Handles SIGTERM/SIGINT gracefully

## 📊 Comparison: Different Daemon Methods

| Method | Command | Pros | Cons |
|--------|---------|------|------|
| **Built-in `-d`** | `sk_server 8080 -d` | ✅ Simple<br>✅ Built-in<br>✅ No dependencies | ⚠️ Unix only<br>⚠️ Basic features |
| **systemd** | `systemctl start skillet-server` | ✅ Full service management<br>✅ Auto-restart<br>✅ Resource limits | ❌ Linux only<br>❌ Requires setup |
| **Docker** | `docker-compose up -d` | ✅ Portable<br>✅ Isolated<br>✅ Easy scaling | ❌ Requires Docker<br>❌ Resource overhead |
| **PM2** | `pm2 start sk_server` | ✅ Monitoring<br>✅ Log management<br>✅ Clustering | ❌ Requires Node.js<br>❌ Additional dependency |

## ⚠️ Platform Support

- **✅ Linux**: Full daemon support
- **✅ macOS**: Full daemon support  
- **❌ Windows**: Not supported (use Docker or process managers)

## 🐛 Troubleshooting

### Daemon Won't Start
```bash
# Check for errors (daemon might exit immediately)
./target/release/sk_server 8080 -d --pid-file test.pid
sleep 1
if [ ! -f test.pid ]; then
    echo "Daemon failed to start - check permissions and port availability"
fi
```

### Port Already in Use
```bash
# Check what's using the port
lsof -i :8080

# Use different port
./target/release/sk_server 8081 -d
```

### Permission Denied
```bash
# Make sure binary is executable
chmod +x ./target/release/sk_server

# Check if PID file directory is writable
./target/release/sk_server 8080 -d --pid-file /tmp/skillet.pid
```

### Process Won't Stop
```bash
# Check if PID file contains valid PID
cat skillet-server.pid

# Force kill if graceful stop fails
kill -9 $(cat skillet-server.pid)
```

## 🎯 Best Practices

### Production Deployment
```bash
# Use specific PID file location
./target/release/sk_server 8080 8 -d --pid-file /var/run/skillet/server.pid

# Create systemd service wrapper (combines benefits)
# See DAEMON_DEPLOYMENT_GUIDE.md for details
```

### Multiple Instances
```bash
# Different ports and PID files
./target/release/sk_server 8080 4 -d --pid-file skillet-8080.pid
./target/release/sk_server 8081 4 -d --pid-file skillet-8081.pid
./target/release/sk_server 8082 4 -d --pid-file skillet-8082.pid
```

### Monitoring
```bash
#!/bin/bash
# Simple monitoring script
PID_FILE="skillet-server.pid"

if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if kill -0 $PID 2>/dev/null; then
        echo "✅ Skillet server running (PID: $PID)"
    else
        echo "❌ Skillet server not running (stale PID file)"
        rm -f "$PID_FILE"
    fi
else
    echo "❌ Skillet server not running (no PID file)"
fi
```

## 🚀 Quick Demo

Test the daemon functionality:

```bash
# Run the demo script
./demo_daemon_flag.sh
```

This will:
- Test normal vs daemon mode
- Verify proper daemonization
- Test functionality
- Show process management
- Demonstrate signal handling

The built-in `-d` flag makes it super easy to run Skillet server as a daemon without any external dependencies!