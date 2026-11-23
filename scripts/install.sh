#!/bin/bash
# Skillet Installation Script
# Downloads and installs the latest Skillet binaries

set -e

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux*)
        if [ "$ARCH" = "x86_64" ]; then
            PLATFORM="linux-x86_64"
        elif [ "$ARCH" = "aarch64" ]; then
            PLATFORM="linux-arm64"
        else
            echo "Unsupported architecture: $ARCH"
            exit 1
        fi
        ;;
    Darwin*)
        if [ "$ARCH" = "x86_64" ]; then
            PLATFORM="macos-x86_64"
        elif [ "$ARCH" = "arm64" ]; then
            PLATFORM="macos-arm64"
        else
            echo "Unsupported architecture: $ARCH"
            exit 1
        fi
        ;;
    *)
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac

REPO="zenbakiak/skillet"
ARCHIVE="skillet-${PLATFORM}.tar.gz"
URL="https://github.com/${REPO}/releases/latest/download/${ARCHIVE}"

echo "🔍 Detected platform: $PLATFORM"
echo "📥 Downloading latest Skillet from: $URL"

# Download
curl -L -o "/tmp/${ARCHIVE}" "$URL"

# Extract
echo "📦 Extracting binaries..."
tar -xzf "/tmp/${ARCHIVE}" -C /tmp

# Install (default to /usr/local/bin, can be overridden)
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
echo "📂 Installing to: $INSTALL_DIR"

sudo mv /tmp/sk "$INSTALL_DIR/sk"
sudo mv /tmp/sk_server "$INSTALL_DIR/sk_server" 2>/dev/null || true
sudo mv /tmp/sk_client "$INSTALL_DIR/sk_client" 2>/dev/null || true
sudo mv /tmp/sk_http_server "$INSTALL_DIR/sk_http_server" 2>/dev/null || true
sudo mv /tmp/sk_http_bench "$INSTALL_DIR/sk_http_bench" 2>/dev/null || true

sudo chmod +x "$INSTALL_DIR"/sk*

# Cleanup
rm "/tmp/${ARCHIVE}"

echo "✅ Skillet installed successfully!"
echo ""
echo "Try it: sk '2 + 2'"
