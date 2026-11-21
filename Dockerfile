# Multi-stage build for the Skillet HTTP server (sk_http_server).
# Toolchain pinned to the CI/release toolchain (ADR-0012); bump with RUST_TOOLCHAIN in the workflows.
FROM rust:1.89.0-slim-bookworm AS builder

WORKDIR /app
COPY . .

# --locked: build exactly what Cargo.lock pins.
RUN cargo build --release --locked --bin sk_http_server

# Runtime image: same Debian release as the builder, so glibc matches.
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Non-root user. /app stays root-owned; only the hooks directory is writable by the server.
RUN groupadd -r skillet && useradd -r -g skillet -d /app -s /bin/false skillet \
    && mkdir -p /app/hooks \
    && chown skillet:skillet /app/hooks

WORKDIR /app

COPY --from=builder --chown=root:root /app/target/release/sk_http_server ./
COPY --chown=root:root --chmod=0755 docker/entrypoint.sh docker/healthcheck.sh ./

USER skillet

# Bind all interfaces inside the container so `-p` publishing works. Restrict exposure on the
# host side (docker-compose.yml publishes to 127.0.0.1 by default).
ENV PORT=8080 \
    HOST=0.0.0.0 \
    SK_THREADS=2 \
    SKILLET_HOOKS_DIR=/app/hooks

# SKILLET_AUTH_TOKEN and SKILLET_ADMIN_TOKEN are required at run time and have no default;
# entrypoint.sh refuses to start without them.

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["./healthcheck.sh"]

ENTRYPOINT ["./entrypoint.sh"]
