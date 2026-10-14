#!/bin/sh
# The Authorization nginx sends to GitHub for /gh/ (nginx.conf.template
# includes the file this writes). GitHub lists stargazers to authenticated
# callers only.
#
# - A GitHub App: LANDING_GITHUB_APP_CLIENT_ID and
#   LANDING_GITHUB_APP_PRIVATE_KEY_BASE64 (the app's PEM, base64-encoded). The
#   app must be installed on ThunderForgeVTT/ThunderForgeVTT; it needs no
#   permission beyond the metadata every app has. An installation token lasts
#   an hour, so a fresh one is minted every 50 minutes and nginx reloaded.
# - Otherwise GITHUB_TOKEN, as it is.
# - Otherwise nothing: the star chart shows contributors only.
#
# The nginx image runs every executable in /docker-entrypoint.d before nginx.
set -eu
umask 077

OUT=${GITHUB_AUTH_CONF:-/etc/nginx/github-auth.conf}
API=https://api.github.com
REPO=ThunderForgeVTT/ThunderForgeVTT

log() { echo "github-auth: $*" >&2; }

write() {
  printf 'map "" $github_auth {\n  default "%s";\n}\n' "$1" >"$OUT.tmp"
  chmod 644 "$OUT.tmp"
  mv "$OUT.tmp" "$OUT"
}

b64url() { base64 | tr -d '\n=' | tr '/+' '_-'; }

# A JWT for the app, RS256. iat is a minute back for clock drift; GitHub
# refuses an exp more than ten minutes out.
app_jwt() {
  now=$(date +%s)
  head=$(printf '{"alg":"RS256","typ":"JWT"}' | b64url)
  body=$(printf '{"iat":%d,"exp":%d,"iss":"%s"}' $((now - 60)) $((now + 540)) "$LANDING_GITHUB_APP_CLIENT_ID" | b64url)
  key=$(mktemp)
  if ! printf '%s' "$LANDING_GITHUB_APP_PRIVATE_KEY_BASE64" | base64 -d >"$key" 2>/dev/null; then
    rm -f "$key"
    log "LANDING_GITHUB_APP_PRIVATE_KEY_BASE64 is not base64"
    return 1
  fi
  sig=$(printf '%s.%s' "$head" "$body" | openssl dgst -sha256 -sign "$key" -binary | b64url) || sig=
  rm -f "$key"
  [ -n "$sig" ] || { log "the private key could not sign"; return 1; }
  printf '%s.%s.%s' "$head" "$body" "$sig"
}

github() { # METHOD PATH JWT [BODY]
  curl -sS --max-time 10 -X "$1" \
    -H "Authorization: Bearer $3" \
    -H "Accept: application/vnd.github+json" \
    -H "User-Agent: thunderforge.dev" \
    ${4:+-d "$4"} "$API$2"
}

# An installation token for the one repository, read-only metadata only.
mint() {
  jwt=$(app_jwt) || return 1
  found=$(github GET "/repos/$REPO/installation" "$jwt") || { log "GitHub did not answer"; return 1; }
  id=$(printf '%s' "$found" | tr -d '\n' | sed -n 's/^{ *"id": *\([0-9][0-9]*\).*/\1/p')
  [ -n "$id" ] || { log "no installation on $REPO: $(printf '%s' "$found" | tr -d '\n' | cut -c1-200)"; return 1; }
  made=$(github POST "/app/installations/$id/access_tokens" "$jwt" \
    "{\"repositories\":[\"${REPO#*/}\"],\"permissions\":{\"metadata\":\"read\"}}") || { log "GitHub did not answer"; return 1; }
  token=$(printf '%s' "$made" | tr -d '\n' | sed -n 's/.*"token": *"\([A-Za-z0-9_]*\)".*/\1/p')
  [ -n "$token" ] || { log "no token: $(printf '%s' "$made" | tr -d '\n' | cut -c1-200)"; return 1; }
  printf '%s' "$token"
}

if [ -n "${LANDING_GITHUB_APP_CLIENT_ID:-}" ] && [ -n "${LANDING_GITHUB_APP_PRIVATE_KEY_BASE64:-}" ]; then
  if token=$(mint); then
    write "Bearer $token"
    log "an installation token for $REPO"
    wait=3000
  else
    write ""
    log "no token yet; trying again in a minute"
    wait=60
  fi
  # Refresh in the background for the container's life; an old token is kept
  # until a new one is in hand.
  (
    while sleep "$wait"; do
      if token=$(mint); then
        write "Bearer $token"
        nginx -s reload 2>/dev/null || log "nginx did not reload"
        wait=3000
      else
        wait=60
      fi
    done
  ) &
elif [ -n "${GITHUB_TOKEN:-}" ]; then
  write "Bearer $GITHUB_TOKEN"
else
  write ""
fi
