#!/bin/bash

# Unified Skillet Server Daemon Manager
# Works across Linux (systemd), macOS (launchd), and Docker

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_status() { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Detect platform and service system
detect_platform() {
    if command -v docker &> /dev/null && [[ -f "docker-compose.yml" ]]; then
        echo "docker"
    elif [[ "$OSTYPE" == "linux-gnu"* ]] && command -v systemctl &> /dev/null; then
        echo "systemd"
    elif [[ "$OSTYPE" == "darwin"* ]] && command -v launchctl &> /dev/null; then
        echo "launchd"
    elif command -v pm2 &> /dev/null; then
        echo "pm2"
    elif command -v supervisorctl &> /dev/null; then
        echo "supervisor"
    else
        echo "unknown"
    fi
}

PLATFORM=$(detect_platform)
SERVICE_NAME="skillet-server"

# Command functions for different platforms
systemd_start() {
    sudo systemctl start $SERVICE_NAME
}

systemd_stop() {
    sudo systemctl stop $SERVICE_NAME
}

systemd_restart() {
    sudo systemctl restart $SERVICE_NAME
}

systemd_status() {
    sudo systemctl status $SERVICE_NAME
}

systemd_logs() {
    sudo journalctl -u $SERVICE_NAME -f
}

systemd_enable() {
    sudo systemctl enable $SERVICE_NAME
}

systemd_disable() {
    sudo systemctl disable $SERVICE_NAME
}

launchd_start() {
    sudo launchctl load /Library/LaunchDaemons/com.$SERVICE_NAME.plist
}

launchd_stop() {
    sudo launchctl unload /Library/LaunchDaemons/com.$SERVICE_NAME.plist
}

launchd_restart() {
    launchd_stop
    sleep 2
    launchd_start
}

launchd_status() {
    if sudo launchctl list | grep -q $SERVICE_NAME; then
        echo "● $SERVICE_NAME is running"
        sudo launchctl list | grep $SERVICE_NAME
    else
        echo "● $SERVICE_NAME is not running"
    fi
}

launchd_logs() {
    tail -f /opt/skillet/logs/skillet-server.log
}

launchd_enable() {
    print_success "Service is automatically enabled with launchd"
}

launchd_disable() {
    print_warning "To disable, unload the service: ./daemon_manager.sh stop"
}

docker_start() {
    docker-compose up -d skillet-server
}

docker_stop() {
    docker-compose stop skillet-server
}

docker_restart() {
    docker-compose restart skillet-server
}

docker_status() {
    docker-compose ps skillet-server
}

docker_logs() {
    docker-compose logs -f skillet-server
}

docker_enable() {
    # Set restart policy to unless-stopped (already in compose file)
    print_success "Docker service will auto-restart unless manually stopped"
}

docker_disable() {
    docker-compose stop skillet-server
    print_success "Docker service stopped and won't auto-restart"
}

pm2_start() {
    pm2 start target/release/sk_server --name $SERVICE_NAME -- 8080 4
}

pm2_stop() {
    pm2 stop $SERVICE_NAME
}

pm2_restart() {
    pm2 restart $SERVICE_NAME
}

pm2_status() {
    pm2 info $SERVICE_NAME
}

pm2_logs() {
    pm2 logs $SERVICE_NAME
}

pm2_enable() {
    pm2 startup
    pm2 save
}

pm2_disable() {
    pm2 delete $SERVICE_NAME
}

supervisor_start() {
    sudo supervisorctl start $SERVICE_NAME
}

supervisor_stop() {
    sudo supervisorctl stop $SERVICE_NAME
}

supervisor_restart() {
    sudo supervisorctl restart $SERVICE_NAME
}

supervisor_status() {
    sudo supervisorctl status $SERVICE_NAME
}

supervisor_logs() {
    sudo supervisorctl tail -f $SERVICE_NAME
}

supervisor_enable() {
    print_success "Supervisor service is automatically enabled"
}

supervisor_disable() {
    print_warning "Edit /etc/supervisor/conf.d/skillet-server.conf and set autostart=false"
}

# Generic command dispatcher
run_command() {
    local cmd=$1
    
    case $PLATFORM in
        systemd)
            systemd_${cmd}
            ;;
        launchd)
            launchd_${cmd}
            ;;
        docker)
            docker_${cmd}
            ;;
        pm2)
            pm2_${cmd}
            ;;
        supervisor)
            supervisor_${cmd}
            ;;
        unknown)
            print_error "Unknown platform or service system"
            exit 1
            ;;
    esac
}

# Health check function
health_check() {
    local host=${1:-localhost}
    local port=${2:-8080}
    local timeout=${3:-5}
    
    print_status "Performing health check on $host:$port..."
    
    if timeout $timeout bash -c "echo '{\"expression\": \"=1+1\", \"variables\": null}' | nc $host $port" > /dev/null 2>&1; then
        local result=$(echo '{"expression": "=42", "variables": null}' | timeout $timeout nc $host $port 2>/dev/null)
        if echo "$result" | grep -q '"success":true'; then
            print_success "✅ Server is healthy and responding correctly"
            return 0
        else
            print_warning "⚠️  Server responding but evaluation failed"
            echo "Response: $result"
            return 1
        fi
    else
        print_error "❌ Server not responding on $host:$port"
        return 2
    fi
}

# Install function
install_daemon() {
    print_status "Installing Skillet Server daemon using $PLATFORM..."
    
    case $PLATFORM in
        systemd|launchd)
            if [[ -f "scripts/install_daemon.sh" ]]; then
                ./scripts/install_daemon.sh
            else
                print_error "install_daemon.sh script not found"
                exit 1
            fi
            ;;
        docker)
            print_status "Building and starting Docker container..."
            docker-compose up -d --build skillet-server
            print_success "Docker container deployed"
            ;;
        pm2)
            print_status "Installing PM2 service..."
            pm2 start target/release/sk_server --name $SERVICE_NAME -- 8080 4
            pm2 startup
            pm2 save
            print_success "PM2 service installed"
            ;;
        supervisor)
            print_error "Supervisor installation requires manual setup. See DAEMON_DEPLOYMENT_GUIDE.md"
            exit 1
            ;;
        unknown)
            print_error "Cannot auto-install on this platform"
            exit 1
            ;;
    esac
}

# Main command handler
case "${1:-help}" in
    start)
        print_status "Starting Skillet Server daemon ($PLATFORM)..."
        run_command start
        sleep 2
        health_check
        ;;
    stop)
        print_status "Stopping Skillet Server daemon ($PLATFORM)..."
        run_command stop
        print_success "Service stopped"
        ;;
    restart)
        print_status "Restarting Skillet Server daemon ($PLATFORM)..."
        run_command restart
        sleep 3
        health_check
        ;;
    status)
        print_status "Checking Skillet Server status ($PLATFORM)..."
        run_command status
        ;;
    logs)
        print_status "Showing Skillet Server logs ($PLATFORM)..."
        run_command logs
        ;;
    enable)
        print_status "Enabling Skillet Server auto-start ($PLATFORM)..."
        run_command enable
        ;;
    disable)
        print_status "Disabling Skillet Server auto-start ($PLATFORM)..."
        run_command disable
        ;;
    health|check)
        health_check "${2:-localhost}" "${3:-8080}" "${4:-5}"
        ;;
    install)
        install_daemon
        ;;
    info)
        echo "🚀 Skillet Server Daemon Manager"
        echo "================================"
        echo "Detected platform: $PLATFORM"
        echo "Service name: $SERVICE_NAME"
        echo ""
        case $PLATFORM in
            systemd)
                echo "Management commands:"
                echo "  sudo systemctl start/stop/restart $SERVICE_NAME"
                echo "  sudo systemctl status $SERVICE_NAME"
                echo "  sudo journalctl -u $SERVICE_NAME -f"
                ;;
            launchd)
                echo "Management commands:"
                echo "  sudo launchctl load/unload /Library/LaunchDaemons/com.$SERVICE_NAME.plist"
                echo "  sudo launchctl list | grep $SERVICE_NAME"
                ;;
            docker)
                echo "Management commands:"
                echo "  docker-compose up/down/restart skillet-server"
                echo "  docker-compose logs -f skillet-server"
                ;;
            pm2)
                echo "Management commands:"
                echo "  pm2 start/stop/restart $SERVICE_NAME"
                echo "  pm2 logs $SERVICE_NAME"
                ;;
            supervisor)
                echo "Management commands:"
                echo "  sudo supervisorctl start/stop/restart $SERVICE_NAME"
                echo "  sudo supervisorctl tail -f $SERVICE_NAME"
                ;;
        esac
        ;;
    build)
        print_status "Building Skillet Server..."
        cargo build --release --bin sk_server
        print_success "Build completed"
        ;;
    help|*)
        echo "🚀 Skillet Server Daemon Manager"
        echo "================================"
        echo ""
        echo "Detected platform: $PLATFORM"
        echo ""
        echo "Usage: $0 <command> [options]"
        echo ""
        echo "Commands:"
        echo "  install             Install daemon service"
        echo "  start               Start the service"
        echo "  stop                Stop the service"
        echo "  restart             Restart the service"
        echo "  status              Show service status"
        echo "  logs                Follow service logs"
        echo "  enable              Enable auto-start on boot"
        echo "  disable             Disable auto-start"
        echo "  health [host] [port] [timeout]  Health check (default: localhost 8080 5s)"
        echo "  build               Build the server binary"
        echo "  info                Show platform-specific info"
        echo "  help                Show this help"
        echo ""
        echo "Examples:"
        echo "  $0 install          # Install as daemon"
        echo "  $0 start            # Start service"
        echo "  $0 health           # Check if server is healthy"
        echo "  $0 logs             # Follow logs"
        ;;
esac