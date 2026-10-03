#!/bin/sh
# Bring the schema up to date, then become the server.
#
# Migrations run here rather than in a sidecar for one reason: there is exactly
# one app container, so there is no second writer to race, and a separate
# service would need its own copy of the same two files to do the same work.
# If the schema cannot be applied the instance must not start — a server
# answering requests against a half-migrated database is worse than a server
# that is plainly down — so `set -e` is the whole error policy.
set -eu

cd /srv/thunderforge/migrate
echo "thunderforge: applying migrations"
diesel migration run

echo "thunderforge: starting server on 0.0.0.0:30000"
exec /usr/local/bin/thunderforge \
  --ip-address 0.0.0.0 \
  --port 30000 \
  --data-path "${THUNDERFORGE_DATA_PATH:-/srv/thunderforge/data}"
