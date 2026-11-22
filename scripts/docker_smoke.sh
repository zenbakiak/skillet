#!/usr/bin/env bash
# Smoke test for the sk_http_server image (BL-062). CI (rust.yml, "docker image") and humans run this.
#
# Usage: scripts/docker_smoke.sh [--build] [IMAGE]
#   IMAGE    image to test (default: skillet-http:smoke)
#   --build  first build IMAGE from ./Dockerfile with `docker build`
# Env: SMOKE_PORT (host port on 127.0.0.1, default 18080), SMOKE_TIMEOUT (seconds, default 60).
# Needs docker and curl. Exit 0 = all checks passed, 1 = a check failed.
set -euo pipefail
cd "$(dirname "$0")/.."

build=0
if [[ "${1:-}" == "--build" ]]; then
    build=1
    shift
fi
image="${1:-skillet-http:smoke}"
port="${SMOKE_PORT:-18080}"
timeout="${SMOKE_TIMEOUT:-60}"
base="http://127.0.0.1:${port}"
name="skillet-smoke-$$"

# Fresh random tokens per run. They reach docker by name only (`-e NAME`), never on argv.
token() { od -An -N16 -tx1 /dev/urandom | tr -d ' \n'; }
SKILLET_AUTH_TOKEN="$(token)"
SKILLET_ADMIN_TOKEN="$(token)"
export SKILLET_AUTH_TOKEN SKILLET_ADMIN_TOKEN

cleanup() {
    docker rm -f "$name" "$name-a" "$name-b" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Container log with both token values masked, for failure output.
redacted_logs() {
    local logs
    logs="$(docker logs "$name" 2>&1 || true)"
    logs="${logs//"$SKILLET_AUTH_TOKEN"/<auth-token>}"
    printf '%s\n' "${logs//"$SKILLET_ADMIN_TOKEN"/<admin-token>}"
}

ok() { printf 'ok   - %s\n' "$*"; }
fail() {
    printf 'FAIL - %s\n' "$*" >&2
    if docker inspect "$name" >/dev/null 2>&1; then
        printf -- '--- container log (tokens redacted) ---\n' >&2
        redacted_logs >&2
    fi
    exit 1
}

# Runs the image expecting a refusal to start: exit 64 and `$2` on stderr.
expect_refusal() {
    local label="$1" want="$2" err rc=0
    shift 2
    err="$(docker run --rm --name "$name-$label" "$@" "$image" 2>&1 >/dev/null)" || rc=$?
    [[ "$rc" == 64 ]] || fail "($label) expected exit 64, got $rc; stderr: $err"
    [[ "$err" == *"$want"* ]] || fail "($label) stderr does not mention '$want': $err"
}

if [[ "$build" == 1 ]]; then
    docker build -t "$image" .
fi
docker image inspect "$image" >/dev/null 2>&1 || fail "image $image not found (build it, or pass --build)"

# (a) No tokens: refuse to start and name the missing variable.
expect_refusal a "SKILLET_AUTH_TOKEN"
ok "(a) no tokens: exit 64, stderr names SKILLET_AUTH_TOKEN"

# (b) Only the pre-BL-051 names: refuse, with the rename hint.
expect_refusal b "rename them to SKILLET_AUTH_TOKEN and SKILLET_ADMIN_TOKEN" \
    -e AUTH_TOKEN=legacy-auth -e ADMIN_TOKEN=legacy-admin
ok "(b) only AUTH_TOKEN/ADMIN_TOKEN: exit 64 with the rename hint"

# (c) Both tokens: starts, becomes healthy, enforces auth, evaluates.
docker run -d --name "$name" -p "127.0.0.1:${port}:8080" \
    -e SKILLET_AUTH_TOKEN -e SKILLET_ADMIN_TOKEN "$image" >/dev/null

deadline=$((SECONDS + timeout))
ready=""
while [[ -z "$ready" ]]; do
    state="$(docker inspect -f '{{.State.Status}} {{if .State.Health}}{{.State.Health.Status}}{{end}}' "$name")"
    [[ "$state" == running* ]] || fail "(c) container is not running (state: $state)"
    if [[ "$state" == "running healthy" ]]; then
        ready="docker health check is healthy"
    elif [[ "$(curl -s -o /dev/null -w '%{http_code}' "$base/health" || true)" == 200 ]]; then
        ready="GET /health returned 200"
    elif ((SECONDS >= deadline)); then
        fail "(c) not ready after ${timeout}s (state: $state)"
    else
        sleep 1
    fi
done
ok "(c) started: $ready"

expr='%3D2%2B3' # =2+3
code="$(curl -s -o /dev/null -w '%{http_code}' "$base/eval?expr=$expr")"
[[ "$code" == 401 ]] || fail "(c) /eval without a token returned $code, want 401"
ok "(c) /eval without a token: 401"

# The header goes through curl's stdin config, so the token is not on curl's argv either.
body="$(printf 'header = "Authorization: Bearer %s"\n' "$SKILLET_AUTH_TOKEN" |
    curl -sS --config - "$base/eval?expr=$expr")"
[[ "$body" =~ \"success\":true ]] || fail "(c) /eval with the token did not succeed: $body"
[[ "$body" =~ \"result\":5(\.0+)?[,}] ]] || fail "(c) =2+3 did not give 5: $body"
ok "(c) /eval with the token: =2+3 gives 5"

# (d) Neither token value appears in the container log.
logs="$(docker logs "$name" 2>&1)"
[[ "$logs" != *"$SKILLET_AUTH_TOKEN"* ]] || fail "(d) SKILLET_AUTH_TOKEN value is in the container log"
[[ "$logs" != *"$SKILLET_ADMIN_TOKEN"* ]] || fail "(d) SKILLET_ADMIN_TOKEN value is in the container log"
ok "(d) container log contains neither token value"

printf 'docker smoke: all checks passed (%s)\n' "$image"
