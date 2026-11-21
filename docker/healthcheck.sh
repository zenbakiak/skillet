#!/bin/bash
# HEALTHCHECK for the sk_http_server image: GET /health over bash's /dev/tcp, so the runtime
# image needs no curl or wget. Exit 0 on an HTTP 200 status line, 1 otherwise.
# Docker's --timeout bounds how long this may take.
set -eu

port="${PORT:-8080}"
exec 3<>"/dev/tcp/127.0.0.1/${port}"
printf 'GET /health HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n' >&3
IFS= read -r status <&3 || exit 1
case "$status" in
    "HTTP/1."?" 200"*) exit 0 ;;
    *) exit 1 ;;
esac
