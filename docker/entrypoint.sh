#!/bin/sh
# Entrypoint for the sk_http_server container image (BL-051).
#
# Fails closed: the container will not start unless both SKILLET_AUTH_TOKEN and
# SKILLET_ADMIN_TOKEN are set and non-empty. sk_http_server reads both tokens from the
# environment, so they never appear on argv (visible in `ps` and `docker inspect`) and this
# script never prints them.
set -eu

missing=""
if [ -z "${SKILLET_AUTH_TOKEN:-}" ]; then
    missing="SKILLET_AUTH_TOKEN"
fi
if [ -z "${SKILLET_ADMIN_TOKEN:-}" ]; then
    missing="${missing:+$missing and }SKILLET_ADMIN_TOKEN"
fi

if [ -n "$missing" ]; then
    # Images before BL-051 used AUTH_TOKEN/ADMIN_TOKEN. Point upgraders at the new names.
    if [ -n "${AUTH_TOKEN+set}${ADMIN_TOKEN+set}" ]; then
        echo "skillet: refusing to start: $missing must be set to a non-empty value. AUTH_TOKEN and ADMIN_TOKEN are no longer read; rename them to SKILLET_AUTH_TOKEN and SKILLET_ADMIN_TOKEN (both required, non-empty)." >&2
    else
        echo "skillet: refusing to start: $missing must be set to a non-empty value. Set both SKILLET_AUTH_TOKEN and SKILLET_ADMIN_TOKEN to secrets (for example: openssl rand -hex 32)." >&2
    fi
    exit 64
fi

# Drop the legacy names so they cannot be mistaken for the live configuration.
unset AUTH_TOKEN ADMIN_TOKEN 2>/dev/null || true

port="${PORT:-8080}"
host="${HOST:-0.0.0.0}"
threads="${SK_THREADS:-4}"

echo "skillet: starting sk_http_server on ${host}:${port} with ${threads} threads (eval and admin token auth enabled)" >&2
exec ./sk_http_server "$port" --host "$host" --threads "$threads" "$@"
