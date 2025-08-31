#!/bin/bash

# Skillet Server Daemon Installation Script
# Supports systemd (Linux) and launchd (macOS)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_status() { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Configuration
INSTALL_DIR="/opt/skillet"
SERVICE_NAME="skillet-server"
PORT="${SKILLET_PORT:-8080}"
THREADS="${SKILLET_THREADS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)}"
USER="skillet"

# Detect platform
PLATFORM=""
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    PLATFORM="linux"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    PLATFORM="macos"
else
    print_error "Unsupported platform: $OSTYPE"
    exit 1
fi

print_status "Installing Skillet Server as daemon on $PLATFORM"
print_status "Configuration:"
echo "  Install directory: $INSTALL_DIR"
echo "  Port: $PORT"
echo "  Threads: $THREADS"
echo "  User: $USER"
echo ""

# Check if running as root
if [[ $EUID -ne 0 ]]; then
   print_error "This script must be run as root (use sudo)"
   exit 1
fi

# Build the server if not exists
if [[ ! -f "target/release/sk_server" ]]; then
    print_status "Building Skillet server..."
    sudo -u $(logname) cargo build --release --bin sk_server
    print_success "Build completed"
fi

# Create user and directories
print_status "Setting up user and directories..."
if ! id "$USER" &>/dev/null; then
    if [[ "$PLATFORM" == "linux" ]]; then
        useradd --system --shell /bin/false --home-dir $INSTALL_DIR $USER
    elif [[ "$PLATFORM" == "macos" ]]; then
        # Create user on macOS
        dscl . -create /Users/$USER
        dscl . -create /Users/$USER UserShell /bin/false
        dscl . -create /Users/$USER RealName "Skillet Server"
        dscl . -create /Users/$USER UniqueID "505"
        dscl . -create /Users/$USER PrimaryGroupID 20
        dscl . -create /Users/$USER NFSHomeDirectory $INSTALL_DIR
    fi
    print_success "Created user: $USER"
else
    print_warning "User $USER already exists"
fi

# Create directories
mkdir -p $INSTALL_DIR/{logs,hooks,config}
chown -R $USER:$(id -gn $USER) $INSTALL_DIR

# Copy binary
print_status "Installing binary..."
cp target/release/sk_server $INSTALL_DIR/
chown $USER:$(id -gn $USER) $INSTALL_DIR/sk_server
chmod +x $INSTALL_DIR/sk_server
print_success "Binary installed to $INSTALL_DIR/sk_server"

# Create service configuration based on platform
if [[ "$PLATFORM" == "linux" ]]; then
    print_status "Creating systemd service..."
    
    cat > /etc/systemd/system/$SERVICE_NAME.service << EOF
[Unit]
Description=Skillet High-Performance Expression Server
After=network.target
Wants=network.target

[Service]
Type=simple
User=$USER
Group=$USER
WorkingDirectory=$INSTALL_DIR
ExecStart=$INSTALL_DIR/sk_server $PORT $THREADS
ExecReload=/bin/kill -HUP \$MAINPID
KillMode=mixed
KillSignal=SIGTERM
TimeoutStopSec=10
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=$SERVICE_NAME

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=$INSTALL_DIR/logs

# Resource limits
LimitNOFILE=65536
MemoryMax=1G
CPUQuota=400%

# Environment
Environment=RUST_LOG=info
Environment=SKILLET_HOOKS_DIR=$INSTALL_DIR/hooks

[Install]
WantedBy=multi-user.target
EOF

    # Reload and enable service
    systemctl daemon-reload
    systemctl enable $SERVICE_NAME
    print_success "systemd service created and enabled"
    
elif [[ "$PLATFORM" == "macos" ]]; then
    print_status "Creating launchd service..."
    
    cat > /Library/LaunchDaemons/com.$SERVICE_NAME.plist << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" 
"http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.$SERVICE_NAME</string>
    
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_DIR/sk_server</string>
        <string>$PORT</string>
        <string>$THREADS</string>
    </array>
    
    <key>WorkingDirectory</key>
    <string>$INSTALL_DIR</string>
    
    <key>RunAtLoad</key>
    <true/>
    
    <key>KeepAlive</key>
    <true/>
    
    <key>StandardOutPath</key>
    <string>$INSTALL_DIR/logs/skillet-server.log</string>
    
    <key>StandardErrorPath</key>
    <string>$INSTALL_DIR/logs/skillet-server-error.log</string>
    
    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
        <key>SKILLET_HOOKS_DIR</key>
        <string>$INSTALL_DIR/hooks</string>
    </dict>
    
    <key>ThrottleInterval</key>
    <integer>10</integer>
</dict>
</plist>
EOF

    chown root:wheel /Library/LaunchDaemons/com.$SERVICE_NAME.plist
    chmod 644 /Library/LaunchDaemons/com.$SERVICE_NAME.plist
    print_success "launchd service created"
fi

# Create management scripts
print_status "Creating management scripts..."

# Start script
cat > $INSTALL_DIR/start.sh << 'EOF'
#!/bin/bash
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    sudo systemctl start skillet-server
elif [[ "$OSTYPE" == "darwin"* ]]; then
    sudo launchctl load /Library/LaunchDaemons/com.skillet-server.plist
fi
EOF

# Stop script  
cat > $INSTALL_DIR/stop.sh << 'EOF'
#!/bin/bash
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    sudo systemctl stop skillet-server
elif [[ "$OSTYPE" == "darwin"* ]]; then
    sudo launchctl unload /Library/LaunchDaemons/com.skillet-server.plist
fi
EOF

# Status script
cat > $INSTALL_DIR/status.sh << 'EOF'
#!/bin/bash
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    sudo systemctl status skillet-server
elif [[ "$OSTYPE" == "darwin"* ]]; then
    sudo launchctl list | grep skillet-server
fi
EOF

# Logs script
cat > $INSTALL_DIR/logs.sh << 'EOF'
#!/bin/bash
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    sudo journalctl -u skillet-server -f
elif [[ "$OSTYPE" == "darwin"* ]]; then
    tail -f /opt/skillet/logs/skillet-server.log
fi
EOF

# Make scripts executable
chmod +x $INSTALL_DIR/*.sh
chown $USER:$(id -gn $USER) $INSTALL_DIR/*.sh

# Create health check script
cat > $INSTALL_DIR/health-check.sh << EOF
#!/bin/bash
HOST=\${1:-localhost}
PORT=\${2:-$PORT}
TIMEOUT=\${3:-5}

if ! timeout \$TIMEOUT bash -c "echo '{\"expression\": \"=1+1\", \"variables\": null}' | nc \$HOST \$PORT" > /dev/null 2>&1; then
    echo "CRITICAL: Skillet server not responding on \$HOST:\$PORT"
    exit 2
fi

RESULT=\$(echo '{"expression": "=42", "variables": null}' | timeout \$TIMEOUT nc \$HOST \$PORT 2>/dev/null)

if echo "\$RESULT" | grep -q '"success":true'; then
    echo "OK: Skillet server healthy"
    exit 0
else
    echo "WARNING: Skillet server responding but evaluation failed"
    echo "Response: \$RESULT"
    exit 1
fi
EOF

chmod +x $INSTALL_DIR/health-check.sh
chown $USER:$(id -gn $USER) $INSTALL_DIR/health-check.sh

# Create configuration file
cat > $INSTALL_DIR/config/server.conf << EOF
# Skillet Server Configuration
PORT=$PORT
THREADS=$THREADS
LOG_LEVEL=info
HOOKS_DIR=$INSTALL_DIR/hooks
BIND_ADDRESS=0.0.0.0
EOF

chown $USER:$(id -gn $USER) $INSTALL_DIR/config/server.conf

print_success "Installation completed!"
print_status ""
print_status "🚀 Skillet Server has been installed as a daemon service"
print_status ""
print_status "Management commands:"
if [[ "$PLATFORM" == "linux" ]]; then
    echo "  Start:   sudo systemctl start $SERVICE_NAME"
    echo "  Stop:    sudo systemctl stop $SERVICE_NAME"
    echo "  Restart: sudo systemctl restart $SERVICE_NAME" 
    echo "  Status:  sudo systemctl status $SERVICE_NAME"
    echo "  Logs:    sudo journalctl -u $SERVICE_NAME -f"
elif [[ "$PLATFORM" == "macos" ]]; then
    echo "  Start:   sudo launchctl load /Library/LaunchDaemons/com.$SERVICE_NAME.plist"
    echo "  Stop:    sudo launchctl unload /Library/LaunchDaemons/com.$SERVICE_NAME.plist"
    echo "  Status:  sudo launchctl list | grep $SERVICE_NAME"
    echo "  Logs:    tail -f $INSTALL_DIR/logs/skillet-server.log"
fi

echo ""
echo "Convenience scripts (in $INSTALL_DIR):"
echo "  ./start.sh      - Start the service"
echo "  ./stop.sh       - Stop the service"
echo "  ./status.sh     - Check service status"
echo "  ./logs.sh       - View logs"
echo "  ./health-check.sh - Test server health"
echo ""
echo "Configuration:"
echo "  Config file: $INSTALL_DIR/config/server.conf"
echo "  Logs dir:    $INSTALL_DIR/logs/"
echo "  Hooks dir:   $INSTALL_DIR/hooks/"
echo ""

# Ask if user wants to start the service now
read -p "Start the service now? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    print_status "Starting $SERVICE_NAME service..."
    
    if [[ "$PLATFORM" == "linux" ]]; then
        systemctl start $SERVICE_NAME
        sleep 2
        if systemctl is-active --quiet $SERVICE_NAME; then
            print_success "Service started successfully!"
            print_status "Server should be available at http://localhost:$PORT"
        else
            print_error "Service failed to start. Check logs with: sudo journalctl -u $SERVICE_NAME"
        fi
    elif [[ "$PLATFORM" == "macos" ]]; then
        launchctl load /Library/LaunchDaemons/com.$SERVICE_NAME.plist
        sleep 2
        print_success "Service started successfully!"
        print_status "Server should be available at http://localhost:$PORT"
    fi
    
    # Test the server
    print_status "Testing server..."
    sleep 1
    if $INSTALL_DIR/health-check.sh localhost $PORT 2; then
        print_success "✅ Server is responding correctly!"
    else
        print_warning "⚠️  Server may not be ready yet. Check logs or try again in a few seconds."
    fi
fi

print_success "🎉 Skillet Server daemon installation complete!"