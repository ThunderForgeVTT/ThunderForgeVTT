# syntax=docker/dockerfile:1.7
#
# The whole of ThunderForge, built from source, in one image.
#
#   docker compose up -d --build
#
# is the entire install. The `build` stage below compiles everything the
# repository contains — the Bevy engine and the PDF reader to WebAssembly, the
# React client to a static bundle, and the Axum server to a release binary —
# and the runtime stage packages only what runs: the binary, diesel-cli for
# migrations, the game-system and interface packs it reads at boot, and the
# client bundle as a directory beside them.
#
# # The client is a directory, not part of the binary
#
# The server serves the bundle from `STATIC_DIR`, a path read at runtime, so
# the same binary runs with any client build and a rebuilt client does not
# mean a recompiled server. It sits outside the data directory on purpose:
# that directory is where an instance's volumes mount, and a volume over
# `data/` would hide a client stored inside it.
#
# There used to be a second, nginx image in front, because the server could
# not serve its own client: `/assets` was the upload directory alone, so the
# bundle's scripts were 404s, and an unknown path was a JSON 404 rather than
# the client. Both are fixed in `static_files`, and one process is the app.
#
# # Build time
#
# A cold build is long — tens of minutes on a laptop — because the workspace's
# release profile is `lto = true, codegen-units = 1` and `wasm-opt` is
# single-threaded over a ~25MB module. Neither scales with cores. cargo-chef
# keeps every compiled dependency in a layer of its own (the `cook` stage), so
# a rebuild after a source change compiles only this workspace's crates, and
# BUILD_PROFILE=dev (what `make push` uses) skips LTO and wasm-opt altogether.

ARG RUST_VERSION=1.98
ARG NODE_VERSION=24
ARG PNPM_VERSION=10.33.2
ARG WASM_PACK_VERSION=0.15.0
ARG DIESEL_CLI_VERSION=2.3.13
ARG CARGO_CHEF_VERSION=0.1.78
# `release` for anything a visitor downloads; `dev` for the dev cluster, where
# the wait matters more than the size: an unoptimised engine (no wasm-opt) and
# a debug server, minutes instead of tens of minutes. `make push` passes `dev`
# unless CI=true.
ARG BUILD_PROFILE=release

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
ARG CARGO_CHEF_VERSION

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
# cargo-chef splits the Rust build so dependencies are a layer of their own;
# see the `cook` stage.
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=thunderforge-cargo-registry \
  cargo install wasm-pack --version "${WASM_PACK_VERSION}" --locked \
  && cargo install diesel_cli --version "${DIESEL_CLI_VERSION}" --locked \
  --no-default-features --features postgres \
  && cargo install cargo-chef --version "${CARGO_CHEF_VERSION}" --locked

# --- the dependencies (cargo-chef) -------------------------------------------
#
# `planner` reduces the workspace to its manifests and lockfile (recipe.json);
# `cook` compiles every dependency from that recipe alone. The cooked `target/`
# and the registry it was fetched from are ordinary layers, so they stay valid
# until a dependency changes, a source edit never invalidates them, and unlike
# a cache mount they can be exported (`--cache-to`) and pushed: a CI job run in
# the `cook` image starts with every dependency compiled, at these paths, with
# this rustc and this glibc.
#
# Each cook below has to build its crate exactly as the build stage will —
# same target, profile and features — or cargo sees different dependencies and
# compiles them again. The engine follows BUILD_PROFILE (with `debug-names` on
# dev, as `scripts/shared.mjs` builds it); dice, pdf and combat are always
# release with `wasm`; the server follows BUILD_PROFILE with its defaults,
# which is also what its demo-maps binary needs (that feature enables nothing
# in a dependency).
FROM toolchain AS planner
WORKDIR /build
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM toolchain AS cook
ARG BUILD_PROFILE
WORKDIR /build
# The engine follows BUILD_PROFILE: `release` is what a release is; `dev` is
# a fast, unoptimised bundle for the dev cluster.
ENV ENGINE_PROFILE=${BUILD_PROFILE} \
  CARGO_TERM_COLOR=never \
  CI=true
# `[env]` in the cargo config is part of what a build sees.
COPY .cargo .cargo
COPY --from=planner /build/recipe.json recipe.json
RUN if [ "$BUILD_PROFILE" = dev ]; then flag=""; engine="--features debug-names"; else flag=--release; engine=""; fi \
  && cargo chef cook $flag --recipe-path recipe.json -p thunderforge \
  && cargo chef cook $flag --recipe-path recipe.json --target wasm32-unknown-unknown \
  -p thunderforge-engine $engine \
  && for crate in thunderforge-dice thunderforge-pdf thunderforge-combat; do \
  cargo chef cook --release --recipe-path recipe.json --target wasm32-unknown-unknown \
  -p "$crate" --features wasm || exit 1; \
  done

# --- the build ----------------------------------------------------------------
FROM cook AS build
ARG BUILD_PROFILE

COPY . .

# Order matters. `dist/engine`, `dist/pdf` and `dist/dice` are pnpm workspace
# packages that wasm-pack writes, and `apps/web` and `apps/demo` depend on
# them, so they have to exist before `pnpm install` can resolve the
# workspace. `target/` is the cooked one from the stage above: only this
# workspace's own crates compile here.
RUN --mount=type=cache,target=/root/.cache,id=thunderforge-wasm-pack-cache \
  node scripts/build.mjs --only-wasm

RUN --mount=type=cache,target=/root/.local/share/pnpm/store,id=thunderforge-pnpm-store \
  pnpm install --frozen-lockfile

# The client bundle, written to data/client.
RUN pnpm -F @thunderforge/web run build

# The server in BUILD_PROFILE, stripped: a debug binary carries about 1.4GB
# of symbols and nothing in a container needs them. The same step runs the
# demo's map importer, a second binary of the same crate, in the same profile
# so it reuses what the server build compiled.
RUN if [ "$BUILD_PROFILE" = dev ]; then flag=""; dir=debug; else flag=--release; dir=release; fi \
  && cargo build $flag -p thunderforge \
  && mkdir -p /out \
  && install -m 755 target/$dir/thunderforge /out/thunderforge \
  && strip /out/thunderforge \
  && install -m 755 /usr/local/cargo/bin/diesel /out/diesel \
  && strip /out/diesel \
  && cargo run $flag -q -p thunderforge --bin thunderforge-demo-maps \
  --features demo-maps -- examples/maps apps/demo/public/maps

# The demo (spec 074), written to data/demo: static files and nothing else.
# Its scenes are the example maps the step above just imported, which is why
# it is built after the server and not beside the client.
RUN pnpm -F @thunderforge/demo run build

# --- the homepage ------------------------------------------------------------
#
# thunderforge.dev: apps/landing at / and the demo at /demo/, served by nginx.
# Build it with `--target landing`; the server stays the default (last) stage.
FROM build AS landing-build
RUN pnpm -F @thunderforge/landing run build

FROM nginx:stable-alpine AS landing
# The image fills in the template at start; an empty token is allowed and
# leaves the star chart to contributors only.
ENV GITHUB_TOKEN=""
COPY apps/landing/nginx.conf.template /etc/nginx/templates/default.conf.template
COPY --from=landing-build /build/apps/landing/dist /usr/share/nginx/html
COPY --from=build /build/data/demo /usr/share/nginx/html/demo
EXPOSE 8080

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
COPY crates/thunderforge-server/diesel.toml /srv/thunderforge/migrate/diesel.toml
COPY crates/thunderforge-server/migrations /srv/thunderforge/migrate/migrations

# The game-system and interface packs the server lists at boot — `packs/` in
# the repository is what `scripts/dev.mjs` symlinks into `data/packs/` on a
# developer machine. Their writable siblings — assets, worlds, databases,
# config, modules — are volumes in compose.yml, so an image rebuild does not
# discard an instance's state.
COPY packs /srv/thunderforge/data/packs

# The built client. See the note at the top of this file on why it is here
# and not under the data directory.
COPY --from=build /build/data/client /srv/thunderforge/client

# The built demo (spec 074 FR-016). Served at `/demo` only while the instance
# setting `feature.demo` is on; off by default.
COPY --from=build /build/data/demo /srv/thunderforge/demo

COPY scripts/container-entrypoint.sh /usr/local/bin/thunderforge-entrypoint
# Belt and braces: COPY carries the host's mode, and a script that arrives
# without the execute bit fails at `docker run`, not at `docker build`.
RUN chmod 0755 /usr/local/bin/thunderforge-entrypoint

ENV THUNDERFORGE_DATA_PATH=/srv/thunderforge/data \
  STATIC_DIR=/srv/thunderforge/client \
  DEMO_DIR=/srv/thunderforge/demo
EXPOSE 30000

ENTRYPOINT ["/usr/local/bin/thunderforge-entrypoint"]
