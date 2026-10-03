# The explorable instance, in two images.
#
# Neither stage compiles anything. They package what a build on the host has
# already produced — the `thunderforge` binary, and the client bundle vite
# writes to `data/client` — because compiling this workspace inside a build
# stage would take an order of magnitude longer than the thing it is for:
# standing the whole app up on one port so somebody can click through it.
#
# `make container` is the supported way to build them; it stages the binaries
# into `target/container/` first, which is what the COPYs below expect.
#
# # Why the client is a second image rather than the server's static mount
#
# `static_files::router` does mount `data/client` as the server's fallback, so
# a single container looks like it should work. It does not, for two reasons
# that have gone unnoticed because every development and test run serves the
# frontend from vite instead:
#
#   1. The bundle asks for `/assets/entry/*.js`, `/assets/chunks/*.js` and
#      `/assets/static/*.css`, and `/assets` is `nest_service`d to the *upload*
#      directory — so every script and stylesheet 404s.
#   2. `main.rs` applies `.fallback(errors::handler_404)` after merging that
#      router, which replaces the client fallback, so `/` 404s as JSON too.
#
# Both are worth fixing in the server. Neither is this file's business, and a
# reverse proxy in front is what a deployment wants regardless: it is the one
# place that can try the bundle first and hand everything else to the backend,
# which is exactly what the `/assets` collision needs.

# --- the server ------------------------------------------------------------
#
# The base is ubuntu:24.04 deliberately: the binary is linked against the
# host's glibc (2.39) and libpq, so the runtime has to be the same family. A
# slimmer base with an older glibc produces an image that builds and then
# refuses to start.
FROM ubuntu:24.04 AS server

RUN apt-get update \
  && apt-get install --no-install-recommends --yes \
  ca-certificates \
  curl \
  git \
  libpq5 \
  && rm -rf /var/lib/apt/lists/*

# The server, and the migration tool that has to run before it.
#
# Migrations are not embedded in the binary — `make migrate` shells out to
# diesel-cli — so the container carries the same tool rather than inventing a
# second way to apply them.
COPY target/container/thunderforge /usr/local/bin/thunderforge
COPY target/container/diesel /usr/local/bin/diesel

# diesel-cli reads one directory per invocation, named relative to the config
# it finds in the working directory. Keeping that pair together in its own
# place means the entrypoint does not have to reproduce the repository layout.
COPY src/server/diesel.toml /srv/thunderforge/migrate/diesel.toml
COPY src/server/migrations /srv/thunderforge/migrate/migrations

# The game-system and interface packs the server reads at boot. Their writable
# siblings — assets, worlds, databases, config, modules — are volumes in the
# compose file, so an image rebuild does not discard an instance's state.
COPY data/packs /srv/thunderforge/data/packs

COPY scripts/container-entrypoint.sh /usr/local/bin/thunderforge-entrypoint
# Belt and braces: COPY carries the host's mode, and a script that arrives
# without the execute bit fails at `docker run`, not at `docker build`.
RUN chmod 0755 /usr/local/bin/thunderforge-entrypoint

ENV THUNDERFORGE_DATA_PATH=/srv/thunderforge/data
EXPOSE 30000

ENTRYPOINT ["/usr/local/bin/thunderforge-entrypoint"]

# --- the client ------------------------------------------------------------
FROM nginx:1.27-alpine AS web

COPY scripts/container-nginx.conf /etc/nginx/conf.d/default.conf
COPY data/client /usr/share/nginx/html
