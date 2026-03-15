# Multi-stage build for Skillet HTTP Server
FROM rustlang/rust:nightly-slim as builder

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

# Build the HTTP server binary
RUN cargo build --release --bin sk_http_server

# Runtime image - use sid to match builder's GLIBC 2.39
FROM debian:sid-slim

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


# Environment variables with defaults
ENV DOCKER_PORT=8080
ENV HOST=0.0.0.0
ENV AUTH_TOKEN=""
ENV ADMIN_TOKEN=""
ENV SK_THREADS=2
ENV SKILLET_HOOKS_DIR=/app/hooks

# Create startup script
RUN printf '#!/bin/sh\n\
    set -e\n\
    PORT="${PORT:-${DOCKER_PORT:-8080}}"\n\
    echo "Starting sk_http_server on port $PORT listening on ${HOST}"\n\
    \n\
    # Build command with optional arguments\n\
    CMD="./sk_http_server $PORT --host ${HOST} --threads ${SK_THREADS:-8}"\n\
    if [ -n "$AUTH_TOKEN" ]; then\n\
    CMD="$CMD --token $AUTH_TOKEN"\n\
    fi\n\
    if [ -n "$ADMIN_TOKEN" ]; then\n\
    CMD="$CMD --admin-token $ADMIN_TOKEN"\n\
    fi\n\
    \n\
    echo "Executing: $CMD"\n\
    exec $CMD\n' > start.sh && chmod +x start.sh

# Expose port (Cloud Run uses PORT env var at runtime)
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:${PORT:-${DOCKER_PORT}}/health || exit 1

# Default command
CMD ["./start.sh"]
