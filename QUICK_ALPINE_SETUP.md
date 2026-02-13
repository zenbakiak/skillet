# Quick Alpine Setup - TL;DR

## For Skillet Maintainers: Build Binaries

```bash
# Build Alpine-compatible binaries
bash scripts/build_alpine_binaries.sh

# Binaries will be in: dist/alpine/
# - sk (CLI)
# - sk_http_server (HTTP API)
# - sk_server (TCP server)
# - sk_client (TCP client)
# - sk_http_bench (benchmarking)
```

## For Other Projects: Add to Your Dockerfile

### Simple Method (Copy Pre-built Binaries)

1. Download or copy binaries to your project:
```bash
mkdir -p vendor/skillet
cp /path/to/skillet/dist/alpine/* vendor/skillet/
```

2. Add to your `Dockerfile`:
```dockerfile
FROM ruby:3.4.8-alpine

# Install runtime dependencies
RUN apk add --no-cache ca-certificates curl

# Copy Skillet binaries
COPY vendor/skillet/sk /usr/local/bin/
COPY vendor/skillet/sk_http_server /usr/local/bin/
RUN chmod +x /usr/local/bin/sk*

# Your app setup...
COPY . /app
WORKDIR /app
RUN bundle install

CMD ["rails", "server", "-b", "0.0.0.0"]
```

### Advanced Method (Build from Source)

```dockerfile
# Build stage
FROM rust:1.81-alpine AS skillet-builder
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static git
WORKDIR /skillet
RUN git clone https://github.com/zenbakiak/skillet.git .
ENV RUSTFLAGS="-C target-feature=+crt-static"
RUN cargo build --release --bins

# Runtime stage
FROM ruby:3.4.8-alpine
RUN apk add --no-cache ca-certificates curl
COPY --from=skillet-builder /skillet/target/release/sk* /usr/local/bin/

# Your app...
```

## Using Skillet in Your Ruby App

### HTTP Server (Recommended)

**Start server in background:**
```dockerfile
CMD sh -c 'sk_http_server 5074 & rails server -b 0.0.0.0'
```

**Use from Ruby:**
```ruby
require 'net/http'
require 'json'

def eval_formula(expr, vars = {})
  uri = URI('http://localhost:5074/eval')
  res = Net::HTTP.post(uri, {
    expression: expr,
    arguments: vars
  }.to_json, 'Content-Type' => 'application/json')
  JSON.parse(res.body)['result']
end

# Example
total = eval_formula('SUM(:amounts)', { amounts: [100, 200, 300] })
# => 600
```

### CLI (Quick but slower)

```ruby
def calculate(expr)
  `sk "#{expr}"`.strip
end

calculate("2 + 2 * 3")  # => "8"
```

## Test It Works

```bash
# Build your image
docker build -t myapp .

# Test Skillet
docker run --rm myapp sk "SUM(1,2,3,4,5)"
# Expected output: 15

# Test HTTP server
docker run --rm -p 5074:5074 myapp sk_http_server 5074 --host 0.0.0.0
# In another terminal:
curl "http://localhost:5074/eval?expr=2%2B2"
# Expected: {"result":4}
```

## Complete Example

See `examples/Dockerfile.ruby-alpine-example` for a full Rails integration example.

See `ALPINE_INTEGRATION_GUIDE.md` for detailed documentation.

## Binaries Info

- **All binaries**: Statically linked, no external dependencies
- **Total size**: ~40-50 MB for all binaries
- **Compatible with**: Alpine Linux, ruby:*-alpine, any musl-based system
- **Architecture**: x86_64 (amd64)

## Common Issues

**Binary not found:**
```dockerfile
RUN chmod +x /usr/local/bin/sk*
RUN which sk  # Verify PATH
```

**Missing ca-certificates:**
```dockerfile
RUN apk add --no-cache ca-certificates
```

**Wrong architecture:**
```bash
# For ARM64 Alpine
docker buildx build --platform linux/arm64 ...
```
