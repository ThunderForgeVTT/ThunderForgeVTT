#!/usr/bin/env bash
# Spec 086 SC-009 (R16): the landing's nginx, as the image runs it.
#
# Runs the `landing` image three times, with the defaults, with
# TELEMETRY=false and with a redirected endpoint, and checks:
#   - both served telemetry configs (/telemetry.json, /demo/telemetry.json);
#   - both connect-src values (the landing's and the demo's);
#   - that the access-log line for /?utm_source=x is JSON holding no query,
#     no address, no X-Forwarded-For and no user agent;
#   - that stub_status answers on the pod's loopback and not on 8080.
#
# By default it builds the Dockerfile's `landing` stage, which is the whole
# web build. Two faster ways in:
#   LANDING_IMAGE=<tag>      check an image that is already built;
#   --template-only          check apps/landing/nginx.conf.template in the
#                            stock nginx image, over a stub page. The config
#                            is the same; only the files served differ.
# Skips with a note when Docker is not running.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

if ! docker info >/dev/null 2>&1; then
  echo "check-landing-nginx: Docker is not running, skipped"
  exit 0
fi

TEMPLATE_ONLY=0
[[ "${1:-}" == "--template-only" ]] && TEMPLATE_ONLY=1

STUB=""
RUN_ARGS=()
IMAGE="${LANDING_IMAGE:-}"
if [[ $TEMPLATE_ONLY == 1 ]]; then
  IMAGE="nginx:stable-alpine"
  STUB="$(mktemp -d)"
  mkdir -p "$STUB/demo"
  echo '<!doctype html><title>landing</title>' >"$STUB/index.html"
  echo '<!doctype html><title>demo</title>' >"$STUB/demo/index.html"
  chmod -R a+rX "$STUB"
  RUN_ARGS=(
    -v "$PWD/apps/landing/nginx.conf.template:/etc/nginx/templates/default.conf.template:ro"
    -v "$STUB:/usr/share/nginx/html:ro"
    # The image's defaults, as the Dockerfile's `landing` stage sets them.
    -e GITHUB_TOKEN=
    -e TELEMETRY=true
    -e THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT=https://telemetry.thunderforge.dev
    -e THUNDERFORGE_BROWSER_TELEMETRY_SAMPLE_RATE=1.0
  )
elif [[ -z "$IMAGE" ]]; then
  IMAGE="thunderforge-landing:check"
  docker build --target landing -t "$IMAGE" .
fi

CONTAINER=""
FAILED=0
cleanup() {
  [[ -n "$CONTAINER" ]] && docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
  [[ -n "$STUB" ]] && rm -rf "$STUB"
}
trap cleanup EXIT

fail() {
  echo "  FAIL: $*"
  FAILED=1
}

expect_contains() { # what, haystack, needle
  if [[ "$2" != *"$3"* ]]; then fail "$1: expected to contain [$3], got [$2]"; fi
}
expect_not_contains() {
  if [[ "$2" == *"$3"* ]]; then fail "$1: must not contain [$3], got [$2]"; fi
}

start() { # extra docker args...
  [[ -n "$CONTAINER" ]] && docker rm -f "$CONTAINER" >/dev/null
  CONTAINER="$(docker run -d -p 127.0.0.1::8080 "${RUN_ARGS[@]}" "$@" "$IMAGE")"
  PORT="$(docker port "$CONTAINER" 8080/tcp | head -1 | sed 's/.*://')"
  BASE="http://127.0.0.1:$PORT"
  for _ in $(seq 1 50); do
    curl -fsS "$BASE/healthz" >/dev/null 2>&1 && return 0
    sleep 0.2
  done
  docker logs "$CONTAINER" | tail -20
  fail "nginx did not start"
  exit 1
}

header() { # path, header name
  curl -fsS -D - -o /dev/null "$BASE$1" | tr -d '\r' |
    awk -v h="$(echo "$2" | tr 'A-Z' 'a-z')" -F': ' 'tolower($1)==h {print substr($0, length($1)+3)}'
}

check_run() { # name, landing connect-src, demo connect-src, config needles...
  local name="$1" landing="$2" demo="$3"
  shift 3
  echo "check-landing-nginx: $name"
  for path in /telemetry.json /demo/telemetry.json; do
    local body
    body="$(curl -fsS "$BASE$path")"
    for needle in "$@"; do expect_contains "$name $path" "$body" "$needle"; done
    expect_contains "$name $path cache" "$(header "$path" cache-control)" "no-store"
  done
  expect_contains "$name / connect-src" "$(header / content-security-policy)" "$landing"
  expect_contains "$name /demo/ connect-src" "$(header /demo/ content-security-policy)" "$demo"
}

# 1. The defaults: the project's endpoint, anonymous.
start
check_run "defaults" \
  "connect-src 'self' https://telemetry.thunderforge.dev;" \
  "connect-src 'self' data: blob: https://telemetry.thunderforge.dev" \
  '"enabled":true' '"endpoint":"https://telemetry.thunderforge.dev"' '"tier":"anonymous"' '"sampleRate":1.0'

# The access log: one request with everything a visitor could leak.
curl -fsS -o /dev/null \
  -A "check-landing-agent/1.0" \
  -H "X-Forwarded-For: 203.0.113.9" \
  -e "https://news.example.com/item?id=42" \
  "$BASE/?utm_source=x"
sleep 0.3
LINE="$(docker logs "$CONTAINER" 2>/dev/null | grep '"path":"/"' | tail -1 || true)"
if [[ -z "$LINE" ]]; then
  fail "no access-log line for /"
else
  if ! python3 -c 'import json,sys; d=json.loads(sys.argv[1]); assert d["path"]=="/" and d["status"]==200 and d["referrer_origin"]=="https://news.example.com", d' "$LINE"; then
    fail "the access-log line is not the JSON expected: $LINE"
  fi
  expect_not_contains "access log" "$LINE" "utm_source"
  expect_not_contains "access log" "$LINE" "203.0.113.9"
  expect_not_contains "access log" "$LINE" "check-landing-agent"
  expect_not_contains "access log" "$LINE" "172."
  expect_not_contains "access log" "$LINE" "127.0.0.1"
  expect_not_contains "access log" "$LINE" "item?id"
fi
if docker logs "$CONTAINER" 2>/dev/null | grep -q '"path":"/healthz"'; then
  fail "/healthz is logged"
fi

# stub_status: on the pod's loopback, never on the served port.
if ! docker exec "$CONTAINER" wget -qO- http://127.0.0.1:8081/stub_status | grep -q "Active connections"; then
  fail "stub_status does not answer on 127.0.0.1:8081"
fi
if curl -fsS "$BASE/stub_status" 2>/dev/null | grep -q "Active connections"; then
  fail "stub_status is reachable on 8080"
fi

# 2. Telemetry off.
start -e TELEMETRY=false
check_run "TELEMETRY=false" \
  "connect-src 'self' ;" \
  "connect-src 'self' data: blob:" \
  '{"enabled":false}'
expect_not_contains "TELEMETRY=false /demo/ connect-src" "$(header /demo/ content-security-policy)" "thunderforge.dev"

# 3. Redirected to an operator's collector.
start -e THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT=https://otel.example.org/otlp
check_run "redirected" \
  "connect-src 'self' https://otel.example.org;" \
  "connect-src 'self' data: blob: https://otel.example.org" \
  '"enabled":true' '"endpoint":"https://otel.example.org/otlp"' '"tier":"operator"'

if [[ $FAILED != 0 ]]; then
  echo "check-landing-nginx: FAILED"
  exit 1
fi
echo "check-landing-nginx: ok ($IMAGE)"
