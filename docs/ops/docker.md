# Docker Deployment Guide for Skillet HTTP Server

This guide explains how to run `sk_http_server` from the repository's `Dockerfile`.

The image **fails closed**: it will not start unless both `SKILLET_AUTH_TOKEN` and
`SKILLET_ADMIN_TOKEN` are set to non-empty values. There is no unauthenticated mode in the
image. For local experiments without tokens, run the binary directly (`sk_http_server 5074`),
which binds to `127.0.0.1` by default.

## Quick Start

### 1. Configure tokens

```bash
git clone <your-repo>
cd skillet

# Copy the environment template. .env is gitignored; never commit it.
cp .env.example .env
```

Fill in both tokens in `.env`:

```bash
SKILLET_AUTH_TOKEN=<output of: openssl rand -hex 32>
SKILLET_ADMIN_TOKEN=<output of: openssl rand -hex 32>
DOCKER_PORT=8080
```

`.env` is read by Docker Compose as plain `KEY=value` lines; shell syntax such as `$(...)` is not
expanded there, so paste the generated values in.

### 2. Start with Docker Compose

```bash
docker compose up -d
docker compose logs -f skillet-http

# Published on the host loopback only
curl http://127.0.0.1:8080/health
```

## Configuration

### Environment variables

| Variable | Description | Default | Required |
|----------|-------------|---------|----------|
| `SKILLET_AUTH_TOKEN` | Bearer token for `/eval` and the other non-admin endpoints | _(none)_ | **Yes** |
| `SKILLET_ADMIN_TOKEN` | Bearer token for JS function management and `DELETE /cache` | _(none)_ | **Yes** |
| `PORT` | Port inside the container | `8080` | No |
| `HOST` | Bind address inside the container | `0.0.0.0` | No |
| `SK_THREADS` | Worker threads | `2` | No |
| `SKILLET_HOOKS_DIR` | JS functions directory | `/app/hooks` | No |
| `SKILLET_MAX_CONNECTIONS_PER_IP` | Open connections allowed from one IP address; more get HTTP 429. Passed through to `sk_http_server` as is; `0` or an empty value stops start-up | _(no limit)_ | No |
| `SKILLET_EVAL_TIMEOUT` | Wall-clock limit for one evaluation, whole seconds (1 to 86400). Passed through as is; `0`, empty or invalid stops start-up | `60` | No |
| `SKILLET_MAX_EVAL_BYTES` | Budget of values one evaluation may create, in bytes; `K`, `M`, `G` mean KiB, MiB, GiB. Passed through as is; `0`, empty or invalid stops start-up | `1G` | No |
| `DOCKER_PORT` | Host port (Compose only) | `8080` | No |
| `SKILLET_PUBLISH_ADDR` | Host address the port is published on (Compose only) | `127.0.0.1` | No |

The entrypoint (`docker/entrypoint.sh`) passes the tokens to `sk_http_server` through the
environment only. They are never put on the command line (where `ps` and `docker inspect` would
show them) and never written to the container log.

`SKILLET_MAX_CONNECTIONS_PER_IP`, `SKILLET_EVAL_TIMEOUT` and `SKILLET_MAX_EVAL_BYTES` are not in
`docker-compose.yml`; add them under `environment:` (or pass `-e` to `docker run`) to set them. The
entrypoint does not read them; `sk_http_server` inherits the container environment and reads them
itself, and the start-up log shows the values in use. Behind a reverse proxy every client has the
proxy's address, so set the connection limit at the proxy instead (ADR-0025).

Memory: every worker thread can run one evaluation at the full budget, so plan for about
`SK_THREADS × SKILLET_MAX_EVAL_BYTES` (2 × 1 GiB = 2 GiB with the image defaults; live memory can
reach about twice that, see [limits](../guide/limits.md#sizing-a-server)). Set a container memory
limit above that, or lower the budget or the thread count.

### Network exposure

Inside the container the server binds `0.0.0.0`; that is required for Docker port publishing to
reach it. What the host exposes is decided by the publish address:

- `docker-compose.yml` publishes on `127.0.0.1` by default. Set `SKILLET_PUBLISH_ADDR=0.0.0.0`
  only when the server is behind TLS and a firewall.
- With `docker run`, write the address explicitly: `-p 127.0.0.1:8080:8080`. A bare
  `-p 8080:8080` publishes on every host interface.

## Deployment Options

### Option 1: Docker Compose (recommended)

```bash
# Basic deployment (tokens from .env or the shell environment)
docker compose up -d

# Different host port
DOCKER_PORT=9090 docker compose up -d

# With the Traefik proxy profile
docker compose --profile proxy up -d
```

### Option 2: docker run

```bash
docker build -t skillet-http .

# Tokens come from an env file so they stay out of shell history
docker run -d \
  --name skillet-http-server \
  -p 127.0.0.1:8080:8080 \
  --env-file .env \
  -v ./hooks:/app/hooks:rw \
  skillet-http
```

Without both tokens the container exits immediately with status 64 and one line on stderr:

```text
skillet: refusing to start: SKILLET_AUTH_TOKEN and SKILLET_ADMIN_TOKEN must be set to a non-empty value. ...
```

### Smoke-test an image

`scripts/docker_smoke.sh` runs the checks the CI `docker image` job runs on every push and PR
(BL-062). It needs `docker` and `curl` on the host.

```bash
scripts/docker_smoke.sh --build        # build skillet-http:smoke from ./Dockerfile, then test it
scripts/docker_smoke.sh skillet-http   # test an image you already built
```

It checks that the image exits with 64 when no tokens are set and when only the old
`AUTH_TOKEN`/`ADMIN_TOKEN` are set (with the rename hint); that with both `SKILLET_*` tokens it
becomes healthy, answers `/eval` without a token with 401 and `=2+3` with 5; and that the
container log contains neither token. The tokens are random per run and reach `docker` and `curl`
by environment and stdin, not on the command line. The container is published on
`127.0.0.1:18080` (`SMOKE_PORT` changes it), the wait for health is 60 s (`SMOKE_TIMEOUT`), and
the containers are removed on exit, pass or fail.

## Upgrading from images before BL-051

Older images read `AUTH_TOKEN` and `ADMIN_TOKEN`, treated empty values as "authentication off",
published on all host interfaces, and printed the full command line (tokens included) to the log.
Now:

- Rename `AUTH_TOKEN` to `SKILLET_AUTH_TOKEN` and `ADMIN_TOKEN` to `SKILLET_ADMIN_TOKEN` in
  `.env`, Compose files, and orchestrator secrets. The old names are not read; if they are set and
  the new ones are not, the start-up error says so.
- Both tokens are required.
- Compose publishes on `127.0.0.1`; set `SKILLET_PUBLISH_ADDR` to change that.
- The image has no `curl`. The health check is `./healthcheck.sh`. Images that extended the old
  one and relied on `curl` need to install it themselves.
- The image runs `ENTRYPOINT ["./entrypoint.sh"]`. Extra `docker run` arguments are appended to
  the `sk_http_server` command line.
- Rotate any token that was used with an older image: it is in that container's log.

## Volume Mounts

```bash
# Mount local hooks directory
-v ./hooks:/app/hooks:rw

# Mount from custom location
-v /path/to/your/hooks:/app/hooks:rw
```

## API Usage

```bash
# Health check (no token needed)
curl http://127.0.0.1:8080/health

# Expression evaluation (SKILLET_AUTH_TOKEN)
curl -H "Authorization: Bearer $SKILLET_AUTH_TOKEN" \
  "http://127.0.0.1:8080/eval?expr=2%2B2*3"

# JS function management (SKILLET_ADMIN_TOKEN)
curl -H "Authorization: Bearer $SKILLET_ADMIN_TOKEN" \
  http://127.0.0.1:8080/list-js

# Upload a JS function (SKILLET_ADMIN_TOKEN)
curl -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $SKILLET_ADMIN_TOKEN" \
  -d '{"filename": "custom.js", "js_code": "function DOUBLE(x) { return x * 2; }"}' \
  http://127.0.0.1:8080/upload-js
```

## Production Deployment

### 1. Strong tokens

Generate each token with `openssl rand -hex 32` and keep them in your secret store or a `.env`
file with restricted permissions (`chmod 600 .env`).

### 2. Reverse proxy

```bash
docker compose --profile proxy up -d
# Access via: http://skillet.localhost
# Traefik dashboard (insecure API mode, loopback only): http://127.0.0.1:8090
```

### 3. Persistent storage

```bash
mkdir -p /opt/skillet/hooks
chmod 755 /opt/skillet/hooks
# In docker-compose.yml:
#   - /opt/skillet/hooks:/app/hooks:rw
```

### 4. Resource limits

```yaml
# Add to the docker-compose.yml service
deploy:
  resources:
    limits:
      memory: 512M
      cpus: '0.5'
```

## Security Notes

- Runs as the non-root `skillet` user; the binary and scripts are owned by root and not writable
  by it.
- Base images: `rust:1.89.0-slim-bookworm` (builder, ADR-0012 toolchain) and `debian:bookworm-slim`
  (runtime). The build uses `cargo build --locked`.
- Runtime packages: `ca-certificates` only.
- Rotate tokens by editing `.env` and running `docker compose up -d` (Compose recreates the
  container when its environment changes).

## Monitoring and Logs

```bash
docker compose logs -f skillet-http
docker compose logs --tail 100 skillet-http
docker compose ps          # shows the health status
curl http://127.0.0.1:8080/health
```

`/health` returns basic metrics:

```json
{
  "status": "healthy",
  "version": "0.6.3",
  "requests_processed": 1234,
  "avg_execution_time_ms": 0.5
}
```

## Troubleshooting

### Container exits at start

```bash
docker compose logs skillet-http
```

`refusing to start: ... must be set to a non-empty value` means a token is missing or empty.
Check that `.env` sits next to `docker-compose.yml` and uses the `SKILLET_` names. To check which
variables are set without printing their values:

```bash
docker compose config | grep -E 'SKILLET_(AUTH|ADMIN)_TOKEN' | sed 's/:.*/: <set>/'
```

### Authentication issues

```bash
curl -H "Authorization: Bearer $SKILLET_AUTH_TOKEN" "http://127.0.0.1:8080/eval?expr=1%2B1"
```

### Performance

```bash
docker stats skillet-http-server
curl http://127.0.0.1:8080/health | jq '.avg_execution_time_ms'
```

## Updating

```bash
git pull
docker compose down
docker compose build --no-cache
docker compose up -d
```

### Back up JavaScript functions

```bash
tar -czf hooks-backup-$(date +%Y%m%d).tar.gz hooks/
tar -xzf hooks-backup-20240904.tar.gz
```

## Multi-Instance Deployment

```yaml
# docker-compose.yml
services:
  skillet-http-1:
    build: .
    ports:
      - "127.0.0.1:8081:8080"
    environment:
      SKILLET_AUTH_TOKEN: ${SKILLET_AUTH_TOKEN:-}
      SKILLET_ADMIN_TOKEN: ${SKILLET_ADMIN_TOKEN:-}

  skillet-http-2:
    build: .
    ports:
      - "127.0.0.1:8082:8080"
    environment:
      SKILLET_AUTH_TOKEN: ${SKILLET_AUTH_TOKEN:-}
      SKILLET_ADMIN_TOKEN: ${SKILLET_ADMIN_TOKEN:-}
```

Load balance with nginx or Traefik for production usage.
