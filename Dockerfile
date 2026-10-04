# syntax=docker/dockerfile:1.7
#
# The whole of ThunderForge, built from source, in two images.
#
#   docker compose up -d --build
#
# is the entire install. The `build` stage below compiles everything the
# repository contains — the Bevy engine and the PDF reader to WebAssembly, the
# React client to a static bundle, and the Axum server to a release binary —
# and the two runtime stages package only what runs:
#
#   server   the binary, diesel-cli for migrations, and the game-system and
#            interface packs it reads at boot
#   web      nginx serving the client bundle and proxying `/api` to the server
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
# A reverse proxy in front is what a deployment wants regardless: it is the one
# place that can try the bundle first and hand everything else to the backend,
# which is exactly what the `/assets` collision needs. See
# scripts/container-nginx.conf.
#
# # Build time
#
# A cold build is long — tens of minutes on a laptop — because the workspace's
# release profile is `lto = true, codegen-units = 1` and `wasm-opt` is
# single-threaded over a ~25MB module. Neither scales with cores. BuildKit
# cache mounts keep the cargo registry and the compiled `target/` between
# builds, so a rebuild after a small change is minutes, not tens of minutes.

ARG RUST_VERSION=1.98
ARG NODE_VERSION=24
ARG PNPM_VERSION=10.33.2
ARG WASM_PACK_VERSION=0.15.0
ARG DIESEL_CLI_VERSION=2.3.13

# --- the toolchain -----------------------------------------------------------
#
# Node comes from the official image rather than a distro package so the
# version is the one `.nvmrc`-style tooling expects; pnpm is pinned to the
# `packageManager` field in package.json, which is what the lockfile was made
# with.
FROM node:${NODE_VERSION}-bookworm-slim AS node

FROM rust:${RUST_VERSION}-bookworm AS toolchain
ARG PNPM_VERSION
ARG WASM_PACK_VERSION
ARG DIESEL_CLI_VERSION

RUN apt-get update \
  && apt-get install --no-install-recommends --yes \
  ca-certificates \
  clang \
  curl \
  git \
  libpq-dev \
  pkg-config \
  && rm -rf /var/lib/apt/lists/*

COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules/npm /usr/local/lib/node_modules/npm
RUN ln -s /usr/local/lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
  && npm install --global "pnpm@${PNPM_VERSION}" \
  && node --version && pnpm --version

# The engine and the PDF reader ship as wasm; the server crate needs the host
# target the image already has.
RUN rustup target add wasm32-unknown-unknown

# wasm-pack drives the engine build and fetches a matching wasm-bindgen and
# binaryen on first use. diesel-cli applies the migrations at container start:
# they are not embedded in the binary, and `make migrate` uses the same tool.
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=thunderforge-cargo-registry \
  cargo install wasm-pack --version "${WASM_PACK_VERSION}" --locked \
  && cargo install diesel_cli --version "${DIESEL_CLI_VERSION}" --locked \
  --no-default-features --features postgres

# --- the build ----------------------------------------------------------------
FROM toolchain AS build
WORKDIR /build

# The engine's release profile; `ENGINE_PROFILE=dev` is for a developer who
# wants a fast, unoptimised bundle and is not what a release is.
ENV ENGINE_PROFILE=release \
  CARGO_TERM_COLOR=never \
  CI=true

COPY . .

# Order matters. `dist/engine` and `dist/pdf` are pnpm workspace packages that
# wasm-pack writes, and `apps/web` depends on them, so they have to exist
# before `pnpm install` can resolve the workspace. The cargo `target/` is a
# cache mount: it is shared with the server build below and never lands in an
# image.
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=thunderforge-cargo-registry \
  --mount=type=cache,target=/build/target,id=thunderforge-cargo-target \
  --mount=type=cache,target=/root/.cache,id=thunderforge-wasm-pack-cache \
  node scripts/build.mjs --only-wasm

RUN --mount=type=cache,target=/root/.local/share/pnpm/store,id=thunderforge-pnpm-store \
  pnpm install --frozen-lockfile

# The client bundle, written to data/client.
RUN pnpm -F @thunderforge/web run build

# The server, release profile, stripped: a debug binary carries about 1.4GB of
# symbols and nothing in a container needs them. Copied out of the cache mount
# in the same step, because the mount is gone once the step ends.
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=thunderforge-cargo-registry \
  --mount=type=cache,target=/build/target,id=thunderforge-cargo-target \
  cargo build --release -p thunderforge \
  && mkdir -p /out \
  && install -m 755 target/release/thunderforge /out/thunderforge \
  && strip /out/thunderforge \
  && install -m 755 /usr/local/cargo/bin/diesel /out/diesel \
  && strip /out/diesel

# --- the server ---------------------------------------------------------------
#
# The same Debian release the binary was linked on, so glibc and libpq match.
FROM debian:bookworm-slim AS server

RUN apt-get update \
  && apt-get install --no-install-recommends --yes \
  ca-certificates \
  curl \
  git \
  libpq5 \
  && rm -rf /var/lib/apt/lists/*

COPY --from=build /out/thunderforge /usr/local/bin/thunderforge
COPY --from=build /out/diesel /usr/local/bin/diesel

# diesel-cli reads one directory per invocation, named relative to the config
# it finds in the working directory. Keeping that pair together in its own
# place means the entrypoint does not have to reproduce the repository layout.
COPY src/server/diesel.toml /srv/thunderforge/migrate/diesel.toml
COPY src/server/migrations /srv/thunderforge/migrate/migrations

# The game-system and interface packs the server lists at boot — `packs/` in
# the repository is what `scripts/dev.mjs` symlinks into `data/packs/` on a
# developer machine. Their writable siblings — assets, worlds, databases,
# config, modules — are volumes in compose.yml, so an image rebuild does not
# discard an instance's state.
COPY packs /srv/thunderforge/data/packs

COPY scripts/container-entrypoint.sh /usr/local/bin/thunderforge-entrypoint
# Belt and braces: COPY carries the host's mode, and a script that arrives
# without the execute bit fails at `docker run`, not at `docker build`.
RUN chmod 0755 /usr/local/bin/thunderforge-entrypoint

ENV THUNDERFORGE_DATA_PATH=/srv/thunderforge/data
EXPOSE 30000

ENTRYPOINT ["/usr/local/bin/thunderforge-entrypoint"]

# --- the client ---------------------------------------------------------------
FROM nginx:1.27-alpine AS web

COPY scripts/container-nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /build/data/client /usr/share/nginx/html
