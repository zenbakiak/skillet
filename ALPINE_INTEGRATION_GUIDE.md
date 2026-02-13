# Alpine Linux Integration Guide

This guide shows how to integrate Skillet binaries into Alpine Linux containers, specifically for `ruby:3.4.8-alpine` base images.

## Quick Start

### 1. Build Alpine Binaries

```bash
# Build binaries for Alpine Linux (musl)
bash scripts/build_alpine_binaries.sh
```

This creates static binaries in `dist/alpine/` that work on Alpine Linux and other musl-based systems.

### 2. Test Binaries

```bash
# Test the binaries work on Alpine
docker run --rm -v $(pwd)/dist/alpine:/test ruby:3.4.8-alpine /test/sk "SUM(1,2,3,4,5)"
```

## Integration Methods

### Method 1: Copy Binaries from Build Stage (Recommended)

Create a Dockerfile in your Ruby project:

```dockerfile
# Stage 1: Get pre-built Skillet binaries
FROM ghcr.io/yourusername/skillet-alpine-binaries:latest AS skillet-binaries

# Stage 2: Your Ruby application
FROM ruby:3.4.8-alpine

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    curl \
    tzdata

# Copy Skillet binaries from the build stage
COPY --from=skillet-binaries /binaries/sk /usr/local/bin/sk
COPY --from=skillet-binaries /binaries/sk_http_server /usr/local/bin/sk_http_server
COPY --from=skillet-binaries /binaries/sk_server /usr/local/bin/sk_server
COPY --from=skillet-binaries /binaries/sk_client /usr/local/bin/sk_client

# Verify installation
RUN sk --version || sk "1 + 1"

# Your Ruby application setup
WORKDIR /app
COPY Gemfile Gemfile.lock ./
RUN bundle install

COPY . .

CMD ["rails", "server", "-b", "0.0.0.0"]
```

### Method 2: Copy Pre-built Binaries Directly

If you have the binaries in your project:

```dockerfile
FROM ruby:3.4.8-alpine

# Install minimal runtime dependencies
RUN apk add --no-cache ca-certificates curl

WORKDIR /app

# Copy pre-built Skillet binaries into the image
COPY skillet-binaries/sk /usr/local/bin/sk
COPY skillet-binaries/sk_http_server /usr/local/bin/sk_http_server
COPY skillet-binaries/sk_server /usr/local/bin/sk_server
COPY skillet-binaries/sk_client /usr/local/bin/sk_client

# Make them executable
RUN chmod +x /usr/local/bin/sk*

# Your Ruby application
COPY Gemfile Gemfile.lock ./
RUN bundle install

COPY . .

CMD ["rails", "server", "-b", "0.0.0.0"]
```

### Method 3: Build Skillet During Docker Build

Build Skillet binaries as part of your Docker build:

```dockerfile
# Build stage for Skillet
FROM rust:1.81-alpine AS skillet-builder

RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    git

WORKDIR /skillet

# Clone or copy Skillet source
RUN git clone https://github.com/zenbakiak/skillet.git .
# Or: COPY skillet-source/ .

# Build with static linking
ENV RUSTFLAGS="-C target-feature=+crt-static"
RUN cargo build --release --bins

# Runtime stage
FROM ruby:3.4.8-alpine

RUN apk add --no-cache ca-certificates curl tzdata

WORKDIR /app

# Copy Skillet binaries from builder
COPY --from=skillet-builder /skillet/target/release/sk /usr/local/bin/
COPY --from=skillet-builder /skillet/target/release/sk_http_server /usr/local/bin/
COPY --from=skillet-builder /skillet/target/release/sk_server /usr/local/bin/
COPY --from=skillet-builder /skillet/target/release/sk_client /usr/local/bin/

# Your Ruby application
COPY Gemfile Gemfile.lock ./
RUN bundle install

COPY . .

CMD ["rails", "server", "-b", "0.0.0.0"]
```

## Using Skillet in Your Application

### Option A: Embedded HTTP Server

Run Skillet HTTP server alongside your Ruby app:

```dockerfile
FROM ruby:3.4.8-alpine

# Copy Skillet binaries (use any method above)
COPY --from=skillet-binaries /binaries/sk_http_server /usr/local/bin/

# Your app setup...
WORKDIR /app
COPY . .

# Create startup script
RUN cat > /app/start.sh << 'EOF'
#!/bin/sh
# Start Skillet HTTP server in background
sk_http_server 5074 --host 127.0.0.1 &

# Start your Rails app
bundle exec rails server -b 0.0.0.0 -p 3000
EOF

RUN chmod +x /app/start.sh

EXPOSE 3000

CMD ["/app/start.sh"]
```

Then use it from Ruby:

```ruby
require 'net/http'
require 'json'

def evaluate_expression(expr, variables = {})
  uri = URI('http://localhost:5074/eval')
  request = Net::HTTP::Post.new(uri, 'Content-Type' => 'application/json')
  request.body = {
    expression: expr,
    arguments: variables
  }.to_json

  response = Net::HTTP.start(uri.hostname, uri.port) do |http|
    http.request(request)
  end

  JSON.parse(response.body)
end

# Usage
result = evaluate_expression('SUM(:sales, :bonus)', { sales: 5000, bonus: 1000 })
puts result['result'] # => 6000
```

### Option B: Direct CLI Usage

Use the `sk` command directly:

```ruby
def calculate(expression)
  result = `sk "#{expression}"`.strip
  result.to_f if result =~ /^-?\d+(\.\d+)?$/
end

# Usage
total = calculate("SUM(100, 200, 300)")
# => 600.0
```

### Option C: TCP Server Mode

For high-performance evaluation:

**Dockerfile:**
```dockerfile
FROM ruby:3.4.8-alpine

COPY --from=skillet-binaries /binaries/sk_server /usr/local/bin/
COPY --from=skillet-binaries /binaries/sk_client /usr/local/bin/

# Startup script
RUN cat > /app/start.sh << 'EOF'
#!/bin/sh
sk_server 8080 --host 127.0.0.1 &
bundle exec rails server -b 0.0.0.0 -p 3000
EOF

RUN chmod +x /app/start.sh
CMD ["/app/start.sh"]
```

**Ruby client:**
```ruby
require 'socket'
require 'json'

class SkilletClient
  def initialize(host = 'localhost', port = 8080)
    @host = host
    @port = port
  end

  def evaluate(expression, variables = {})
    socket = TCPSocket.new(@host, @port)

    request = {
      expression: expression,
      variables: variables
    }.to_json

    socket.puts(request)
    result = socket.gets
    socket.close

    JSON.parse(result)
  end
end

# Usage
client = SkilletClient.new
result = client.evaluate('[1,2,3,4].filter(:x > 2).sum()')
```

## Docker Compose Example

For development with both Ruby and Skillet services:

```yaml
version: '3.8'

services:
  skillet:
    image: your-registry/skillet-alpine:latest
    ports:
      - "5074:5074"
    command: sk_http_server 5074 --host 0.0.0.0
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:5074/health"]
      interval: 30s
      timeout: 5s
      retries: 3

  app:
    build: .
    ports:
      - "3000:3000"
    environment:
      - SKILLET_URL=http://skillet:5074
    depends_on:
      - skillet
    command: bundle exec rails server -b 0.0.0.0
```

## Publishing Binaries

### Option 1: GitHub Container Registry

Build and push binaries image:

```bash
# Build the binaries-only image
docker build --target binaries \
  -f Dockerfile.alpine-builder \
  -t ghcr.io/yourusername/skillet-alpine-binaries:latest \
  .

# Push to registry
docker push ghcr.io/yourusername/skillet-alpine-binaries:latest
```

### Option 2: Artifact Storage

Store binaries in your CI/CD artifacts or S3:

```bash
# Create tarball
tar -czf skillet-alpine-binaries.tar.gz -C dist/alpine .

# Upload to S3 (example)
aws s3 cp skillet-alpine-binaries.tar.gz s3://your-bucket/skillet/

# Download in Dockerfile
ADD https://your-bucket.s3.amazonaws.com/skillet/skillet-alpine-binaries.tar.gz /tmp/
RUN tar -xzf /tmp/skillet-alpine-binaries.tar.gz -C /usr/local/bin/
```

## Binary Sizes

Typical sizes for Alpine/musl binaries:

```
sk              ~8 MB    (CLI evaluator)
sk_server       ~9 MB    (TCP server)
sk_client       ~7 MB    (TCP client)
sk_http_server  ~10 MB   (HTTP server with all features)
sk_http_bench   ~8 MB    (HTTP benchmarking tool)
```

## Verification

Test that binaries work correctly:

```bash
# Basic evaluation
docker run --rm -v $(pwd)/dist/alpine:/test ruby:3.4.8-alpine \
  /test/sk "SUM(1, 2, 3, 4, 5)"
# Expected: 15

# Array operations
docker run --rm -v $(pwd)/dist/alpine:/test ruby:3.4.8-alpine \
  /test/sk "[10,20,30].filter(:x > 15).sum()"
# Expected: 50

# HTTP server
docker run --rm -p 5074:5074 -v $(pwd)/dist/alpine:/test ruby:3.4.8-alpine \
  /test/sk_http_server 5074 --host 0.0.0.0

# Test from another terminal
curl "http://localhost:5074/eval?expr=2%2B2"
```

## Troubleshooting

### Binary Not Found

```dockerfile
# Ensure correct paths
RUN ls -la /usr/local/bin/sk*
RUN which sk || echo "sk not in PATH"
```

### Permission Denied

```dockerfile
# Make sure binaries are executable
RUN chmod +x /usr/local/bin/sk*
```

### Missing Dependencies

```dockerfile
# Alpine needs these for Skillet
RUN apk add --no-cache \
    ca-certificates \  # For HTTPS
    curl              # Optional, for testing
```

### Architecture Mismatch

These binaries are built for `x86_64-unknown-linux-musl`. For ARM:

```bash
# Build for ARM64 Alpine
docker buildx build \
  --platform linux/arm64 \
  --target binaries \
  -f Dockerfile.alpine-builder \
  -t skillet-alpine-arm64 \
  .
```

## Best Practices

1. **Use Multi-stage Builds**: Keep final image small by copying only binaries
2. **Pin Versions**: Use specific Ruby and Skillet versions in production
3. **Health Checks**: Monitor Skillet HTTP server if running as service
4. **Resource Limits**: Set memory limits for Skillet processes
5. **Security**: Run as non-root user when possible

```dockerfile
# Example with best practices
FROM ruby:3.4.8-alpine

RUN apk add --no-cache ca-certificates curl && \
    addgroup -g 1000 app && \
    adduser -D -u 1000 -G app app

COPY --from=skillet-binaries --chown=app:app /binaries/sk_http_server /usr/local/bin/

USER app
WORKDIR /app

HEALTHCHECK --interval=30s --timeout=5s \
  CMD curl -f http://localhost:5074/health || exit 1

CMD ["sk_http_server", "5074", "--host", "0.0.0.0"]
```

## Performance Notes

- Alpine binaries are statically linked and have minimal overhead
- HTTP server typically responds in ~3ms per request
- TCP server is faster (~1ms) for high-throughput scenarios
- CLI mode has process startup overhead (~10ms)

For production workloads, use the HTTP or TCP server modes instead of shelling out to the CLI.
