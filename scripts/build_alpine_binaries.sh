#!/bin/bash
# Build Skillet binaries for Alpine Linux (musl)
# These binaries will work on Alpine, ruby:3.4.8-alpine, and other musl-based systems
#
# Builds aarch64 and x86_64 into dist/alpine/<arch>/. Each arch builds natively in a
# `--platform` container; the non-native one runs under emulation (slow, needs binfmt/QEMU,
# which Docker Desktop has). Build one arch only with: ALPINE_ARCHES=x86_64 $0

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ALPINE_ARCHES="${ALPINE_ARCHES:-aarch64 x86_64}"

echo "🏗️  Building Skillet binaries for Alpine Linux (musl): $ALPINE_ARCHES"

# Use custom dockerignore for Alpine builds; restore it on exit, also when a build fails
echo "📋 Preparing Docker context..."
cp "$PROJECT_DIR/.dockerignore" "$PROJECT_DIR/.dockerignore.backup" 2>/dev/null || true
cp "$PROJECT_DIR/.dockerignore.alpine" "$PROJECT_DIR/.dockerignore"
trap 'mv "$PROJECT_DIR/.dockerignore.backup" "$PROJECT_DIR/.dockerignore" 2>/dev/null || true' EXIT

for ARCH in $ALPINE_ARCHES; do
    case "$ARCH" in
        aarch64) PLATFORM=linux/arm64 ;;
        x86_64) PLATFORM=linux/amd64 ;;
        *) echo "unsupported arch: $ARCH (use aarch64 or x86_64)" >&2; exit 1 ;;
    esac
    DIST_DIR="$PROJECT_DIR/dist/alpine/$ARCH"
    IMAGE="skillet-alpine-binaries:$ARCH"
    mkdir -p "$DIST_DIR"

    # Build using Docker multi-stage build
    echo "📦 Building $ARCH binaries with Docker ($PLATFORM)..."
    docker build \
        --platform "$PLATFORM" \
        --target binaries \
        -f "$PROJECT_DIR/Dockerfile.alpine-builder" \
        -t "$IMAGE" \
        "$PROJECT_DIR"

    # Extract binaries from the Docker image
    echo "📤 Extracting binaries..."
    CONTAINER_ID=$(docker create --platform "$PLATFORM" "$IMAGE")
    docker cp "$CONTAINER_ID:/binaries/." "$DIST_DIR/"
    docker rm "$CONTAINER_ID"

    # Make binaries executable
    chmod +x "$DIST_DIR"/*

    echo ""
    echo "Built $ARCH binaries in $DIST_DIR:"
    ls -lh "$DIST_DIR"
    echo ""
    echo "🧪 Testing $ARCH sk binary..."
    docker run --rm --platform "$PLATFORM" -v "$DIST_DIR:/test" ruby:3.4.8-alpine /test/sk "2 + 3 * 4"
    echo ""
done

echo "✅ Alpine Linux binaries built successfully!"
echo "📁 Location: $PROJECT_DIR/dist/alpine/<arch>/"
echo ""
echo "📋 Next steps:"
echo "1. Copy binaries to your project: cp $PROJECT_DIR/dist/alpine/<arch>/* /path/to/your/project/"
echo "2. Or create a tarball: tar -czf skillet-alpine-x86_64.tar.gz -C $PROJECT_DIR/dist/alpine/x86_64 ."
echo "3. See docs/ops/alpine.md for Docker integration instructions"
