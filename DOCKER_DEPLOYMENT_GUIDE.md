# Docker Deployment Guide for Skillet HTTP Server

This guide explains how to deploy the Skillet HTTP Server using Docker with full authentication support.

## Quick Start

### 1. Clone and Configure

```bash
git clone <your-repo>
cd skillet

# Copy the environment template
cp .env.example .env

# Edit .env with your tokens
nano .env
```

### 2. Set Authentication Tokens

Edit `.env` file:
```bash
# Generate secure tokens
AUTH_TOKEN=sk_auth_$(openssl rand -hex 32)
ADMIN_TOKEN=sk_admin_$(openssl rand -hex 32)
DOCKER_PORT=8080
```

### 3. Deploy with Docker Compose

```bash
# Build and start the server
docker-compose up -d

# View logs
docker-compose logs -f skillet-http

# Test the deployment
curl http://localhost:8080/health
```

## Detailed Configuration

### Environment Variables

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `AUTH_TOKEN` | Token for `/eval` endpoints | _(none)_ | No |
| `ADMIN_TOKEN` | Token for JS management | _(none)_ | No |
| `DOCKER_PORT` | External port mapping | `8080` | No |
| `SKILLET_HOOKS_DIR` | JS functions directory | `/app/hooks` | No |

### Authentication Modes

#### 1. No Authentication (Development)
```bash
# .env
AUTH_TOKEN=
ADMIN_TOKEN=
```

#### 2. Eval Authentication Only
```bash
# .env
AUTH_TOKEN=your-secret-token
ADMIN_TOKEN=
```

#### 3. Full Authentication (Recommended)
```bash
# .env
AUTH_TOKEN=your-auth-token
ADMIN_TOKEN=your-admin-token
```

## Deployment Options

### Option 1: Docker Compose (Recommended)

```bash
# Basic deployment
docker-compose up -d

# With custom port
DOCKER_PORT=9090 docker-compose up -d

# With Traefik proxy (for production)
docker-compose --profile proxy up -d
```

### Option 2: Direct Docker Run

```bash
# Build the image
docker build -t skillet-http .

# Run with authentication
docker run -d \
  --name skillet-http-server \
  -p 8080:8080 \
  -e AUTH_TOKEN="your-auth-token" \
  -e ADMIN_TOKEN="your-admin-token" \
  -v ./hooks:/app/hooks:rw \
  skillet-http

# Run without authentication (development)
docker run -d \
  --name skillet-http-server \
  -p 8080:8080 \
  -v ./hooks:/app/hooks:rw \
  skillet-http
```

## Volume Mounts

### JavaScript Functions (hooks)
```bash
# Mount local hooks directory
-v ./hooks:/app/hooks:rw

# Mount from custom location
-v /path/to/your/hooks:/app/hooks:rw
```

## API Usage

### Without Authentication
```bash
# Health check
curl http://localhost:8080/health

# Expression evaluation
curl "http://localhost:8080/eval?expr=2%2B2*3"

# JavaScript function listing
curl http://localhost:8080/list-js
```

### With Authentication
```bash
# Expression evaluation (requires AUTH_TOKEN)
curl -H "Authorization: Bearer your-auth-token" \
  "http://localhost:8080/eval?expr=2%2B2*3"

# JS function management (requires ADMIN_TOKEN)
curl -H "Authorization: Bearer your-admin-token" \
  http://localhost:8080/list-js

# Upload JS function (requires ADMIN_TOKEN)
curl -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-admin-token" \
  -d '{"filename": "custom.js", "js_code": "function DOUBLE(x) { return x * 2; }"}' \
  http://localhost:8080/upload-js
```

## Production Deployment

### 1. Use Strong Tokens
```bash
# Generate cryptographically secure tokens
AUTH_TOKEN=$(openssl rand -hex 32)
ADMIN_TOKEN=$(openssl rand -hex 32)
```

### 2. Enable Reverse Proxy
```bash
# Deploy with Traefik
docker-compose --profile proxy up -d

# Access via: http://skillet.localhost
# Traefik dashboard: http://localhost:8090
```

### 3. Persistent Storage
```bash
# Create persistent hooks directory
mkdir -p /opt/skillet/hooks
chmod 755 /opt/skillet/hooks

# Update docker-compose.yml volume
- /opt/skillet/hooks:/app/hooks:rw
```

### 4. Resource Limits
```yaml
# Add to docker-compose.yml service
deploy:
  resources:
    limits:
      memory: 512M
      cpus: '0.5'
```

## Security Best Practices

### 1. Network Security
- Use reverse proxy (Traefik/nginx) for HTTPS
- Restrict access with firewall rules
- Use strong authentication tokens

### 2. Container Security
- Runs as non-root user (`skillet`)
- Minimal runtime dependencies
- Regular security updates

### 3. Token Management
```bash
# Rotate tokens regularly
docker-compose down
# Update .env with new tokens
docker-compose up -d
```

## Monitoring and Logs

### View Logs
```bash
# Real-time logs
docker-compose logs -f skillet-http

# Last 100 lines
docker-compose logs --tail 100 skillet-http
```

### Health Monitoring
```bash
# Check container health
docker-compose ps

# Manual health check
curl http://localhost:8080/health
```

### Metrics
The server provides basic metrics via `/health` endpoint:
```json
{
  "status": "healthy",
  "version": "0.3.0",
  "requests_processed": 1234,
  "avg_execution_time_ms": 0.5
}
```

## Troubleshooting

### Container Won't Start
```bash
# Check logs
docker-compose logs skillet-http

# Check environment variables
docker exec skillet-http-server env | grep -E "(AUTH_TOKEN|ADMIN_TOKEN|PORT)"
```

### Authentication Issues
```bash
# Verify token format
curl -H "Authorization: Bearer $AUTH_TOKEN" http://localhost:8080/eval?expr=1%2B1

# Check server logs for auth attempts
docker-compose logs skillet-http | grep -i auth
```

### Performance Issues
```bash
# Check container resources
docker stats skillet-http-server

# Monitor request metrics
curl http://localhost:8080/health | jq '.avg_execution_time_ms'
```

## Updating

### Update Server
```bash
# Pull latest changes
git pull

# Rebuild and restart
docker-compose down
docker-compose build --no-cache
docker-compose up -d
```

### Backup JavaScript Functions
```bash
# Backup hooks directory
tar -czf hooks-backup-$(date +%Y%m%d).tar.gz hooks/

# Restore hooks
tar -xzf hooks-backup-20240904.tar.gz
```

## Multi-Instance Deployment

For high availability, deploy multiple instances:

```yaml
# docker-compose.yml
services:
  skillet-http-1:
    build: .
    ports:
      - "8081:8080"
    environment:
      - AUTH_TOKEN=${AUTH_TOKEN}
      - ADMIN_TOKEN=${ADMIN_TOKEN}
    
  skillet-http-2:
    build: .
    ports:
      - "8082:8080"
    environment:
      - AUTH_TOKEN=${AUTH_TOKEN}
      - ADMIN_TOKEN=${ADMIN_TOKEN}
```

Load balance with nginx or Traefik for production usage.