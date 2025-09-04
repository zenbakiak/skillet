#!/bin/bash
set -e

echo "🥘 Skillet HTTP Server Docker Build & Test Script"
echo "================================================"

# Check if Docker is running
if ! docker info >/dev/null 2>&1; then
    echo "❌ Docker is not running. Please start Docker and try again."
    exit 1
fi

# Check if .env exists
if [ ! -f .env ]; then
    echo "📝 Creating .env file from template..."
    cp .env.example .env
    echo "⚠️  Please edit .env with your authentication tokens before deploying!"
fi

# Build the Docker image
echo "🔨 Building Docker image..."
docker build -t skillet-http . || {
    echo "❌ Docker build failed"
    exit 1
}

echo "✅ Docker image built successfully!"

# Test basic compilation
echo "🧪 Testing basic cargo build..."
cargo check --bin sk_http_server || {
    echo "❌ Cargo check failed"
    exit 1
}

echo "✅ Cargo check passed!"

# Show usage instructions
echo ""
echo "🚀 Ready to deploy! Choose one of the following options:"
echo ""
echo "Option 1: Docker Compose (Recommended)"
echo "  docker-compose up -d"
echo ""
echo "Option 2: Direct Docker Run"
echo "  docker run -d -p 8080:8080 --env-file .env -v ./hooks:/app/hooks:rw skillet-http"
echo ""
echo "Option 3: Test without authentication"
echo "  docker run -d -p 8080:8080 -v ./hooks:/app/hooks:rw skillet-http"
echo ""
echo "📖 See DOCKER_DEPLOYMENT_GUIDE.md for detailed instructions"
echo ""
echo "🧪 Test endpoints:"
echo "  curl http://localhost:8080/health"
echo "  curl \"http://localhost:8080/eval?expr=2%2B2*3\""
echo ""