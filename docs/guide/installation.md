# Installation Guide

## Quick Install (Recommended)

Use the automatic installer to get the latest version:

```bash
curl -sSL https://raw.githubusercontent.com/zenbakiak/skillet/main/scripts/install.sh | bash
```

Or download and inspect first:

```bash
curl -sSL -o install.sh https://raw.githubusercontent.com/zenbakiak/skillet/main/scripts/install.sh
chmod +x install.sh
./install.sh
```

## Manual Installation

### macOS

**Apple Silicon (M1/M2/M3):**
```bash
curl -L -o skillet.tar.gz https://github.com/zenbakiak/skillet/releases/latest/download/skillet-macos-arm64.tar.gz
tar -xzf skillet.tar.gz
sudo mv sk* /usr/local/bin/
```

**Intel:**
```bash
curl -L -o skillet.tar.gz https://github.com/zenbakiak/skillet/releases/latest/download/skillet-macos-x86_64.tar.gz
tar -xzf skillet.tar.gz
sudo mv sk* /usr/local/bin/
```

### Linux

**x86_64 (Intel/AMD):**
```bash
curl -L -o skillet.tar.gz https://github.com/zenbakiak/skillet/releases/latest/download/skillet-linux-x86_64.tar.gz
tar -xzf skillet.tar.gz
sudo mv sk* /usr/local/bin/
```

**ARM64:**
```bash
curl -L -o skillet.tar.gz https://github.com/zenbakiak/skillet/releases/latest/download/skillet-linux-arm64.tar.gz
tar -xzf skillet.tar.gz
sudo mv sk* /usr/local/bin/
```

## Verify Installation

```bash
sk "2 + 2"
# Output: Number(4.0)
```

## Available Binaries

- `sk` - CLI evaluator
- `sk_server` - TCP server
- `sk_client` - TCP client
- `sk_http_server` - HTTP API server
- `sk_http_bench` - HTTP benchmarking tool

## Docker (Alpine)

For Alpine Linux or lightweight containers:

```dockerfile
FROM alpine:latest
RUN apk add --no-cache curl
RUN curl -L -o /tmp/skillet.tar.gz https://github.com/zenbakiak/skillet/releases/latest/download/skillet-linux-$(uname -m).tar.gz \
    && tar -xzf /tmp/skillet.tar.gz -C /usr/local/bin/ \
    && rm /tmp/skillet.tar.gz
```

Or use with Ruby:

```dockerfile
FROM ruby:3.4.8-alpine
RUN apk add --no-cache curl
RUN curl -L -o /tmp/skillet.tar.gz https://github.com/zenbakiak/skillet/releases/latest/download/skillet-linux-aarch64.tar.gz \
    && tar -xzf /tmp/skillet.tar.gz -C /usr/local/bin/ \
    && rm /tmp/skillet.tar.gz
```

## Build from Source

```bash
git clone https://github.com/zenbakiak/skillet.git
cd skillet
cargo build --release
sudo cp target/release/sk* /usr/local/bin/
```

## Updating

Just run the install script again to get the latest version:

```bash
curl -sSL https://raw.githubusercontent.com/zenbakiak/skillet/main/scripts/install.sh | bash
```
