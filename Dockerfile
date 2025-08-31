# Multi-stage build for Skillet Server
FROM rust:1.81-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

# Build the server binary
RUN cargo build --release --bin sk_server

# Runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    netcat-openbsd \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r skillet && useradd -r -g skillet -d /app -s /bin/false skillet

# Create directories
RUN mkdir -p /app/{logs,hooks,config} && \
    chown -R skillet:skillet /app

WORKDIR /app
USER skillet

# Copy binary from builder
COPY --from=builder --chown=skillet:skillet /app/target/release/sk_server ./

# Create default config
COPY --chown=skillet:skillet <<EOF /app/config/server.conf
PORT=8080
THREADS=4
LOG_LEVEL=info
BIND_ADDRESS=0.0.0.0
EOF

# Health check script
COPY --chown=skillet:skillet <<'EOF' /app/health-check.sh
#!/bin/bash
echo '{"expression": "=1+1", "variables": null}' | nc localhost 8080 > /dev/null 2>&1
EOF

RUN chmod +x ./health-check.sh

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ./health-check.sh || exit 1

# Default command
CMD ["./sk_server", "8080", "4"]