# Multi-stage build for Skillet HTTP Server
FROM rust:1.81-slim as builder

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

# Build the HTTP server binary
RUN cargo build --release --bin sk_http_server

# Runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r skillet && useradd -r -g skillet -d /app -s /bin/false skillet

# Create directories
RUN mkdir -p /app/hooks && \
    chown -R skillet:skillet /app

WORKDIR /app
USER skillet

# Copy binary from builder
COPY --from=builder --chown=skillet:skillet /app/target/release/sk_http_server ./

# Create health check script
RUN echo '#!/bin/bash\ncurl -f http://localhost:${PORT:-8080}/health > /dev/null 2>&1' > health-check.sh && \
    chmod +x health-check.sh

# Environment variables with defaults
ENV PORT=8080
ENV HOST=0.0.0.0
ENV AUTH_TOKEN="sk-gGAZdgwJrMf7x1qB08yVi3bKVBHjSGyZ"
ENV ADMIN_TOKEN="sk-Drf85SWctwFh45Vc6buxQOU2k6jiEwTr"
ENV SKILLET_HOOKS_DIR=/app/hooks

# Expose port
EXPOSE $PORT

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ./health-check.sh || exit 1

# Default command with environment variable support
CMD sh -c './sk_http_server $PORT --host $HOST \
    ${AUTH_TOKEN:+--token "$AUTH_TOKEN"} \
    ${ADMIN_TOKEN:+--admin-token "$ADMIN_TOKEN"}'