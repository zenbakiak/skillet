# Skillet Server Daemon Deployment Guide

This guide shows how to run the Skillet server as a proper daemon/service for production use.

## Method 1: systemd Service (Linux - Recommended)

### Create Service File

```bash
sudo nano /etc/systemd/system/skillet-server.service
```

```ini
[Unit]
Description=Skillet High-Performance Expression Server
After=network.target
Wants=network.target

[Service]
Type=simple
User=skillet
Group=skillet
WorkingDirectory=/opt/skillet
ExecStart=/opt/skillet/target/release/sk_server 8080 8
ExecReload=/bin/kill -HUP $MAINPID
KillMode=mixed
KillSignal=SIGTERM
TimeoutStopSec=10
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=skillet-server

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/skillet/logs

# Resource limits
LimitNOFILE=65536
MemoryMax=1G
CPUQuota=400%

# Environment
Environment=RUST_LOG=info
Environment=SKILLET_HOOKS_DIR=/opt/skillet/hooks

[Install]
WantedBy=multi-user.target
```

### Setup and Installation

```bash
# Create user and directories
sudo useradd --system --shell /bin/false skillet
sudo mkdir -p /opt/skillet/{logs,hooks,config}
sudo chown -R skillet:skillet /opt/skillet

# Copy binary (build first with cargo build --release)
sudo cp target/release/sk_server /opt/skillet/
sudo chown skillet:skillet /opt/skillet/sk_server
sudo chmod +x /opt/skillet/sk_server

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable skillet-server
sudo systemctl start skillet-server

# Check status
sudo systemctl status skillet-server

# View logs
sudo journalctl -u skillet-server -f
```

### Service Management Commands

```bash
# Start/stop/restart
sudo systemctl start skillet-server
sudo systemctl stop skillet-server
sudo systemctl restart skillet-server

# Enable/disable auto-start
sudo systemctl enable skillet-server
sudo systemctl disable skillet-server

# View logs
sudo journalctl -u skillet-server
sudo journalctl -u skillet-server -f  # Follow logs
sudo journalctl -u skillet-server --since "1 hour ago"

# Check resource usage
systemctl show skillet-server --property=MainPID
ps -p $(systemctl show skillet-server --property=MainPID --value) -o pid,rss,vsz,%cpu,%mem,cmd
```

## Method 2: Docker with Daemon Mode

### Create Dockerfile

```dockerfile
FROM rust:1.81 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin sk_server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --system --shell /bin/false skillet
USER skillet
WORKDIR /app

COPY --from=builder /app/target/release/sk_server .
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD echo '{"expression": "=1+1", "variables": null}' | nc localhost 8080 > /dev/null || exit 1

CMD ["./sk_server", "8080", "4"]
```

### Docker Compose with Daemon

```yaml
# docker-compose.yml
version: '3.8'

services:
  skillet-server:
    build: .
    ports:
      - "8080:8080"
    restart: unless-stopped
    environment:
      - RUST_LOG=info
      - SKILLET_HOOKS_DIR=/app/hooks
    volumes:
      - ./hooks:/app/hooks:ro
      - skillet-logs:/app/logs
    healthcheck:
      test: ["CMD", "sh", "-c", "echo '{\"expression\": \"=1+1\", \"variables\": null}' | nc localhost 8080"]
      interval: 30s
      timeout: 5s
      retries: 3
    deploy:
      resources:
        limits:
          memory: 1G
          cpus: '2.0'
        reservations:
          memory: 256M
          cpus: '0.5'
    logging:
      driver: "json-file"
      options:
        max-size: "100m"
        max-file: "5"

volumes:
  skillet-logs:
```

### Docker Daemon Commands

```bash
# Build and start
docker-compose up -d

# View logs
docker-compose logs -f skillet-server

# Check status
docker-compose ps

# Stop
docker-compose down

# Update and restart
docker-compose build
docker-compose up -d --force-recreate

# Monitor resources
docker stats skillet-server
```

## Method 3: Process Manager (PM2 - Node.js style)

### Install PM2

```bash
npm install -g pm2
```

### Create PM2 Ecosystem File

```javascript
// ecosystem.config.js
module.exports = {
  apps: [{
    name: 'skillet-server',
    script: './target/release/sk_server',
    args: '8080 8',
    cwd: '/opt/skillet',
    instances: 1,
    autorestart: true,
    watch: false,
    max_memory_restart: '1G',
    env: {
      RUST_LOG: 'info',
      SKILLET_HOOKS_DIR: '/opt/skillet/hooks'
    },
    env_production: {
      NODE_ENV: 'production',
      RUST_LOG: 'warn'
    },
    log_file: '/opt/skillet/logs/combined.log',
    out_file: '/opt/skillet/logs/out.log',
    error_file: '/opt/skillet/logs/error.log',
    log_date_format: 'YYYY-MM-DD HH:mm:ss Z',
    min_uptime: '10s',
    max_restarts: 10,
    kill_timeout: 5000
  }]
};
```

### PM2 Commands

```bash
# Start with ecosystem file
pm2 start ecosystem.config.js

# Or start directly
pm2 start target/release/sk_server --name skillet-server -- 8080 8

# Management commands
pm2 list                    # List processes
pm2 info skillet-server     # Show process info
pm2 logs skillet-server     # View logs
pm2 monit                   # Monitor dashboard

# Control
pm2 restart skillet-server
pm2 stop skillet-server
pm2 delete skillet-server

# Auto-startup on boot
pm2 startup
pm2 save

# Monitor and reload
pm2 reload skillet-server   # Zero-downtime reload
```

## Method 4: Supervisor (Python-based)

### Install Supervisor

```bash
sudo apt-get install supervisor  # Debian/Ubuntu
# or
sudo yum install supervisor      # CentOS/RHEL
```

### Create Supervisor Config

```bash
sudo nano /etc/supervisor/conf.d/skillet-server.conf
```

```ini
[program:skillet-server]
command=/opt/skillet/target/release/sk_server 8080 8
directory=/opt/skillet
user=skillet
group=skillet
autostart=true
autorestart=true
startretries=3
redirect_stderr=true
stdout_logfile=/opt/skillet/logs/skillet-server.log
stdout_logfile_maxbytes=100MB
stdout_logfile_backups=5
environment=RUST_LOG=info,SKILLET_HOOKS_DIR=/opt/skillet/hooks
```

### Supervisor Commands

```bash
# Reload configuration
sudo supervisorctl reread
sudo supervisorctl update

# Control service
sudo supervisorctl start skillet-server
sudo supervisorctl stop skillet-server  
sudo supervisorctl restart skillet-server
sudo supervisorctl status skillet-server

# View logs
sudo supervisorctl tail skillet-server
sudo supervisorctl tail -f skillet-server  # Follow logs
```

## Method 5: macOS Daemon (launchd)

### Create Launch Daemon Plist

```bash
sudo nano /Library/LaunchDaemons/com.skillet.server.plist
```

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" 
"http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.skillet.server</string>
    
    <key>ProgramArguments</key>
    <array>
        <string>/opt/skillet/target/release/sk_server</string>
        <string>8080</string>
        <string>4</string>
    </array>
    
    <key>WorkingDirectory</key>
    <string>/opt/skillet</string>
    
    <key>RunAtLoad</key>
    <true/>
    
    <key>KeepAlive</key>
    <true/>
    
    <key>StandardOutPath</key>
    <string>/opt/skillet/logs/skillet-server.log</string>
    
    <key>StandardErrorPath</key>
    <string>/opt/skillet/logs/skillet-server-error.log</string>
    
    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
        <key>SKILLET_HOOKS_DIR</key>
        <string>/opt/skillet/hooks</string>
    </dict>
    
    <key>ThrottleInterval</key>
    <integer>10</integer>
</dict>
</plist>
```

### macOS Daemon Commands

```bash
# Load and start
sudo launchctl load /Library/LaunchDaemons/com.skillet.server.plist
sudo launchctl start com.skillet.server

# Stop and unload
sudo launchctl stop com.skillet.server
sudo launchctl unload /Library/LaunchDaemons/com.skillet.server.plist

# Check status
sudo launchctl list | grep skillet

# View logs
tail -f /opt/skillet/logs/skillet-server.log
```

## Enhanced Server with Daemon Features

Let me create an enhanced version of the server with better daemon support:

### Enhanced Server Binary

```bash
# Create enhanced server with daemon features
cat > src/bin/sk_server_daemon.rs << 'EOF'
use skillet::{evaluate_with_custom, evaluate_with_assignments, Value, JSPluginLoader};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::Instant;
use std::fs::OpenOptions;
use std::path::Path;

// ... (same request/response structs as before)

fn setup_logging() {
    // Create logs directory if it doesn't exist
    std::fs::create_dir_all("logs").ok();
    
    // Setup file logging
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/skillet-server.log")
        .expect("Failed to open log file");
    
    // Initialize logging (you might want to use a proper logging crate)
    eprintln!("Logging to logs/skillet-server.log");
}

fn write_pid_file(pid_file: &str) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Write;
    
    let pid = std::process::id();
    let mut file = File::create(pid_file)?;
    writeln!(file, "{}", pid)?;
    eprintln!("PID {} written to {}", pid, pid_file);
    Ok(())
}

fn setup_signal_handlers() {
    // Setup graceful shutdown on SIGTERM/SIGINT
    // Note: This is simplified - use signal-hook crate for production
    ctrlc::set_handler(move || {
        eprintln!("Received shutdown signal, gracefully stopping...");
        std::process::exit(0);
    }).expect("Error setting Ctrl-C handler");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: sk_server_daemon <port> [threads] [--daemon] [--pid-file <file>]");
        std::process::exit(1);
    }
    
    let port: u16 = args[1].parse().unwrap_or_else(|_| {
        eprintln!("Error: Invalid port number");
        std::process::exit(1);
    });
    
    let num_threads: usize = if args.len() > 2 {
        args[2].parse().unwrap_or_else(|_| num_cpus::get())
    } else {
        num_cpus::get()
    };
    
    // Check for daemon flags
    let daemon_mode = args.iter().any(|arg| arg == "--daemon");
    let pid_file = args.iter()
        .position(|arg| arg == "--pid-file")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.clone())
        .unwrap_or_else(|| "skillet-server.pid".to_string());
    
    if daemon_mode {
        setup_logging();
        setup_signal_handlers();
        write_pid_file(&pid_file).expect("Failed to write PID file");
    }
    
    // ... rest of server implementation
    
    eprintln!("🚀 Skillet Server started on port {} (daemon: {})", port, daemon_mode);
    eprintln!("📊 Worker threads: {}", num_threads);
    if daemon_mode {
        eprintln!("🔧 Running in daemon mode, PID file: {}", pid_file);
    }
    
    // ... server loop implementation
}
EOF
```

## Monitoring and Health Checks

### Create Health Check Script

```bash
#!/bin/bash
# skillet-health-check.sh

HOST=${1:-localhost}
PORT=${2:-8080}
TIMEOUT=${3:-5}

# Test basic connectivity
if ! timeout $TIMEOUT bash -c "echo '{\"expression\": \"=1+1\", \"variables\": null}' | nc $HOST $PORT" > /dev/null 2>&1; then
    echo "CRITICAL: Skillet server not responding on $HOST:$PORT"
    exit 2
fi

# Test actual evaluation
RESULT=$(echo '{"expression": "=42", "variables": null}' | timeout $TIMEOUT nc $HOST $PORT 2>/dev/null)

if echo "$RESULT" | grep -q '"success":true'; then
    echo "OK: Skillet server healthy"
    exit 0
else
    echo "WARNING: Skillet server responding but evaluation failed"
    echo "Response: $RESULT"
    exit 1
fi
```

### Monitoring with Nagios/Icinga

```bash
# /etc/nagios/conf.d/skillet.cfg
define service{
    use                     generic-service
    host_name               production-server
    service_description     Skillet Server
    check_command           check_skillet_server!localhost!8080
}

define command{
    command_name    check_skillet_server
    command_line    /usr/local/bin/skillet-health-check.sh $ARG1$ $ARG2$
}
```

## Log Rotation

### Create logrotate Config

```bash
sudo nano /etc/logrotate.d/skillet-server
```

```
/opt/skillet/logs/*.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    create 0644 skillet skillet
    postrotate
        systemctl reload skillet-server
    endscript
}
```

## Production Deployment Checklist

### Security
- [ ] Run as dedicated user (not root)
- [ ] Configure firewall (only allow necessary ports)
- [ ] Set up SSL/TLS termination (nginx/haproxy)
- [ ] Configure resource limits
- [ ] Enable security updates

### Monitoring
- [ ] Health check endpoint
- [ ] Log aggregation (ELK stack, Fluentd)
- [ ] Metrics collection (Prometheus)
- [ ] Alerting (PagerDuty, Slack)
- [ ] Resource monitoring (CPU, Memory, Disk)

### Backup & Recovery
- [ ] Configuration backup
- [ ] Log retention policy
- [ ] Disaster recovery plan
- [ ] Automated deployment pipeline

### Performance
- [ ] Load balancing (multiple instances)
- [ ] Connection pooling
- [ ] Rate limiting
- [ ] Caching layer (Redis)

This comprehensive daemon setup ensures your Skillet server runs reliably in production with proper monitoring, logging, and management capabilities.