#!/bin/bash
# Build Skillet binaries for Alpine Linux (musl)
# These binaries will work on Alpine, ruby:3.4.8-alpine, and other musl-based systems

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
DIST_DIR="$PROJECT_DIR/dist/alpine"

echo "🏗️  Building Skillet binaries for Alpine Linux (musl)..."

# Create distribution directory
mkdir -p "$DIST_DIR"

# Use custom dockerignore for Alpine builds
echo "📋 Preparing Docker context..."
cp "$PROJECT_DIR/.dockerignore" "$PROJECT_DIR/.dockerignore.backup" 2>/dev/null || true
cp "$PROJECT_DIR/.dockerignore.alpine" "$PROJECT_DIR/.dockerignore"

# Build using Docker multi-stage build
echo "📦 Building binaries with Docker (x86_64/amd64)..."
docker build \
    --platform linux/amd64 \
    --target binaries \
    -f "$PROJECT_DIR/Dockerfile.alpine-builder" \
    -t skillet-alpine-binaries \
    "$PROJECT_DIR"

# Restore original dockerignore
echo "📋 Restoring Docker context..."
mv "$PROJECT_DIR/.dockerignore.backup" "$PROJECT_DIR/.dockerignore" 2>/dev/null || true

# Extract binaries from the Docker image
echo "📤 Extracting binaries..."
CONTAINER_ID=$(docker create skillet-alpine-binaries)
docker cp "$CONTAINER_ID:/binaries/." "$DIST_DIR/"
docker rm "$CONTAINER_ID"

# Make binaries executable
chmod +x "$DIST_DIR"/*

# Display results
echo ""
echo "✅ Alpine Linux binaries built successfully!"
echo "📁 Location: $DIST_DIR"
echo ""
echo "Built binaries:"
ls -lh "$DIST_DIR"
echo ""
echo "🧪 Testing sk binary..."
docker run --rm -v "$DIST_DIR:/test" ruby:3.4.8-alpine /test/sk "2 + 3 * 4"

echo ""
echo "📋 Next steps:"
echo "1. Copy binaries to your project: cp $DIST_DIR/* /path/to/your/project/"
echo "2. Or create a tarball: tar -czf skillet-alpine-binaries.tar.gz -C $DIST_DIR ."
echo "3. See ALPINE_INTEGRATION_GUIDE.md for Docker integration instructions"
