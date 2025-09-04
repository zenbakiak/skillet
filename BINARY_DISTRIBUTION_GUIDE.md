# Binary Distribution Guide

This guide explains how to distribute the `sk_http_server` binary without requiring users to install Rust/Cargo.

## Quick Start

The simplest approach is to build and distribute the release binary:

```bash
# Build optimized binary
cargo build --release --bin sk_http_server

# The binary is now available at:
./target/release/sk_http_server
```

Users can run this binary directly on compatible systems without any additional dependencies.

## Building for Distribution

### 1. Release Build (Recommended)

```bash
cargo build --release --bin sk_http_server
```

This creates an optimized binary at `./target/release/sk_http_server` with:
- Better performance
- Smaller binary size
- All optimizations enabled

### 2. Static Linking (Maximum Portability)

For the most portable binary that works across different systems:

```bash
# For Linux (static linking)
RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-gnu --bin sk_http_server

# For macOS (current platform)
cargo build --release --bin sk_http_server
```

### 3. Cross-compilation for Different Platforms

Build for multiple platforms from a single machine:

```bash
# Install additional targets
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-pc-windows-gnu

# Build for Linux
cargo build --release --target x86_64-unknown-linux-gnu --bin sk_http_server

# Build for Windows
cargo build --release --target x86_64-pc-windows-gnu --bin sk_http_server
```

Binaries will be located at:
- Linux: `./target/x86_64-unknown-linux-gnu/release/sk_http_server`
- Windows: `./target/x86_64-pc-windows-gnu/release/sk_http_server.exe`

## Creating Distribution Packages

### Basic Distribution

```bash
# Create distribution directory
mkdir -p dist

# Copy binary
cp target/release/sk_http_server dist/

# Copy documentation
cp DOCUMENTATION.md dist/
cp SERVER_USAGE_GUIDE.md dist/
cp DAEMON_DEPLOYMENT_GUIDE.md dist/

# Create startup script
cat > dist/start_server.sh << 'EOF'
#!/bin/bash
echo "Starting Skillet HTTP Server..."
./sk_http_server 5074 --host 0.0.0.0
EOF

chmod +x dist/start_server.sh
```

### Distribution with JavaScript Hooks Support

```bash
# Include hooks directory for custom functions
mkdir -p dist/hooks

# Copy existing hooks if any
cp -r hooks/* dist/hooks/ 2>/dev/null || true

# Create example hook
cat > dist/hooks/example.js << 'EOF'
// @name: HELLO
// @min_args: 1
// @max_args: 1
// @description: Returns a greeting
// @example: HELLO("World") returns "Hello, World!"

function execute(args) {
    return "Hello, " + args[0] + "!";
}
EOF

# Create README for hooks
cat > dist/hooks/README.md << 'EOF'
# Custom JavaScript Functions

Place your custom JavaScript functions in this directory.
They will be automatically loaded when the server starts.

See DOCUMENTATION.md for details on creating custom functions.
EOF
```

### Complete Distribution Package

```bash
# Create comprehensive distribution
mkdir -p skillet-http-server

# Copy binary
cp target/release/sk_http_server skillet-http-server/

# Copy documentation
cp DOCUMENTATION.md skillet-http-server/
cp SERVER_USAGE_GUIDE.md skillet-http-server/
cp DAEMON_DEPLOYMENT_GUIDE.md skillet-http-server/
cp DAEMON_FLAG_GUIDE.md skillet-http-server/

# Create hooks directory
mkdir -p skillet-http-server/hooks

# Create startup scripts
cat > skillet-http-server/start.sh << 'EOF'
#!/bin/bash
echo "🚀 Starting Skillet HTTP Server..."
echo "📖 Visit http://localhost:5074 for API documentation"
./sk_http_server 5074
EOF

cat > skillet-http-server/start-public.sh << 'EOF'
#!/bin/bash
echo "🚀 Starting Skillet HTTP Server (public access)..."
echo "⚠️  Server will be accessible from other machines"
echo "📖 Visit http://localhost:5074 for API documentation"
./sk_http_server 5074 --host 0.0.0.0
EOF

cat > skillet-http-server/start-secure.sh << 'EOF'
#!/bin/bash
echo "🚀 Starting Skillet HTTP Server with token authentication..."
echo "🔒 Token: secret123"
echo "📖 Visit http://localhost:5074 for API documentation"
./sk_http_server 5074 --host 0.0.0.0 --token secret123
EOF

chmod +x skillet-http-server/*.sh

# Create README
cat > skillet-http-server/README.md << 'EOF'
# Skillet HTTP Server

A high-performance mathematical and logical expression evaluation server.

## Quick Start

1. Make the binary executable (Linux/macOS):
   ```bash
   chmod +x sk_http_server
   ```

2. Start the server:
   ```bash
   ./sk_http_server 5074
   ```

3. Visit http://localhost:5074 for API documentation

## Startup Scripts

- `start.sh` - Basic server on localhost:5074
- `start-public.sh` - Server accessible from other machines
- `start-secure.sh` - Server with token authentication

## Custom Functions

Place JavaScript files in the `hooks/` directory to add custom functions.
See DOCUMENTATION.md for details.

## Documentation

- `DOCUMENTATION.md` - Complete language and API reference
- `SERVER_USAGE_GUIDE.md` - HTTP server usage examples
- `DAEMON_DEPLOYMENT_GUIDE.md` - Production deployment guide
- `DAEMON_FLAG_GUIDE.md` - Command-line options reference
EOF

# Create archive
tar -czf skillet-http-server.tar.gz skillet-http-server/
echo "✅ Distribution package created: skillet-http-server.tar.gz"
```

## User Installation

Users receive the distribution and can use it immediately:

### Linux/macOS
```bash
# Extract (if archived)
tar -xzf skillet-http-server.tar.gz
cd skillet-http-server

# Make executable
chmod +x sk_http_server

# Run directly
./sk_http_server 5074

# Or use startup scripts
./start.sh
```

### Windows
```cmd
REM Extract the files and run
sk_http_server.exe 5074
```

## System Requirements

### Dependencies Included
The binary includes all necessary dependencies:
- JavaScript runtime (rquickjs)
- HTTP server functionality
- Mathematical and logical operations
- JSON processing
- All built-in functions

### System Requirements
- **Linux**: glibc 2.17+ (most modern distributions)
- **macOS**: 10.12+ (Sierra or newer)
- **Windows**: Windows 7+ (64-bit)

### Runtime Dependencies
- **curl** (optional, for JavaScript HTTP functions)
- No other external dependencies required

## Advanced Distribution Options

### Docker Container
Create a Dockerfile for containerized distribution:

```dockerfile
FROM alpine:latest
RUN apk add --no-cache curl
COPY sk_http_server /usr/local/bin/
COPY hooks/ /app/hooks/
WORKDIR /app
EXPOSE 5074
CMD ["sk_http_server", "5074", "--host", "0.0.0.0"]
```

### Systemd Service
Create a systemd service file for Linux servers:

```ini
[Unit]
Description=Skillet HTTP Server
After=network.target

[Service]
Type=simple
User=skillet
WorkingDirectory=/opt/skillet
ExecStart=/opt/skillet/sk_http_server 5074 --host 0.0.0.0
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

## Binary Properties

### What's Included
- Complete Skillet expression evaluator
- HTTP server with CORS support
- JavaScript plugin system
- Authentication support
- Health monitoring
- All built-in functions (math, logical, text, date, array)

### What's NOT Required
- Rust compiler or Cargo
- External JavaScript runtime
- Additional libraries or frameworks
- Package managers

### Security Considerations
- Binary includes no external network dependencies
- Optional token-based authentication
- No automatic updates (manual deployment)
- Runs with user permissions (not root required)

## Troubleshooting

### Binary Won't Execute
```bash
# Check if executable
ls -la sk_http_server

# Make executable if needed
chmod +x sk_http_server

# Check architecture compatibility
file sk_http_server
```

### Port Already in Use
```bash
# Use different port
./sk_http_server 8080

# Check what's using port 5074
lsof -i :5074  # Linux/macOS
netstat -ano | findstr 5074  # Windows
```

### Permission Denied
```bash
# Don't run as root - use regular user
# Make sure user has permission to bind to port (>1024 recommended)
./sk_http_server 8080 --host 127.0.0.1
```

This approach allows you to distribute a single, self-contained binary that runs anywhere without requiring users to install development tools or manage dependencies.