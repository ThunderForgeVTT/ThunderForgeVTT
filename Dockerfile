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
# compiles every dependency before any source arrives (the `cook` stage), and
# everything cargo writes lives in one BuildKit cache mount that outlives the
# build, so a rebuild after a source change compiles only the workspace crates
# that edit touched. The homepage image (`--target landing`) never builds the
# server: its stages fork off before it. BUILD_PROFILE=dev (what `make push`
# uses) skips LTO and wasm-opt altogether.

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
# `cook` compiles every dependency from that recipe alone, so a source edit
# never recompiles a dependency.
#
# Everything cargo writes goes to one BuildKit cache mount, `target/`, shared
# by every stage below. The mount outlives the build, so the second build of a
# tree compiles only the crates whose sources changed: the workspace's own 39
# crates, which a layer could never keep (the `COPY` that brings the sources
# in makes a new layer, and a new layer starts from the cooked `target/`
# alone). The mount is per machine and `make gc` can trim it; when it is cold
# or gone cargo simply builds what is missing, the cook included, and the
# result is the same.
#
# Each cook below has to build its crate exactly as the build stage will —
# same target, profile and features — or cargo sees different dependencies and
# compiles them again. The engine follows BUILD_PROFILE (with `debug-names` on
# dev, as `scripts/shared.mjs` builds it); dice, pdf and combat are always
# release with `wasm`; the server follows BUILD_PROFILE with its defaults.
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
RUN --mount=type=cache,target=/build/target,id=thunderforge-target \
  if [ "$BUILD_PROFILE" = dev ]; then flag=""; engine="--features debug-names"; else flag=--release; engine=""; fi \
  && cargo chef cook $flag --recipe-path recipe.json -p thunderforge \
  && cargo chef cook $flag --recipe-path recipe.json --target wasm32-unknown-unknown \
  -p thunderforge-engine $engine \
  && for crate in thunderforge-dice thunderforge-pdf thunderforge-combat thunderforge-system-dnd5e-sheet; do \
  cargo chef cook --release --recipe-path recipe.json --target wasm32-unknown-unknown \
  -p "$crate" --features wasm || exit 1; \
  done

# --- the Rust sources ---------------------------------------------------------
#
# Only what cargo reads: the workspace needs every member's manifest and
# sources to resolve, so all of them come in, with the files the server
# crate `include_str!`s from outside its own tree (config, legal, the map
# credit) — but nothing of the web apps, packages or docs. An edit to the
# homepage or the demo leaves these layers, and the wasm and base-maps stages
# built on them, untouched. The same list again in `base-maps`: a stage can
# copy from another but not share a step with it.
FROM cook AS rust-src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY packs packs
COPY config config
COPY legal legal
COPY examples/maps examples/maps
COPY apps/thunderforge apps/thunderforge
COPY apps/thunderforge-crucible-server apps/thunderforge-crucible-server
COPY apps/thunderforge-mapforge-server apps/thunderforge-mapforge-server
COPY apps/telemetry-gateway apps/telemetry-gateway

# --- the wasm ------------------------------------------------------------------
#
# `dist/engine`, `dist/pdf`, `dist/dice`, `dist/combat` and `dist/sheet-*`:
# pnpm workspace packages that wasm-pack writes, which `apps/web`, `apps/demo`
# and `apps/landing` depend on, so they exist before any `pnpm install`.
# `wasm-opt` over the release engine is minutes on its own (it already uses
# every core; whole-module passes are the serial part), which is why this is a
# stage of its own that only a Rust edit re-runs.
#
# And why the outputs are kept between builds: every package in `dist/` carries
# a `pkg.sum` over its sources, both manifests, the lockfile, the profile and
# the build env, and `build.mjs` skips a package whose sum still matches. In an
# image build that check never fired, because `dist/` is in `.dockerignore` and
# each run started from an empty tree — so a server-crate edit, or the same
# commit built from another checkout, paid the engine's wasm-opt again. The
# previous `dist/` is copied in from a cache mount before the build and copied
# back out only after it succeeds, so a failed run never leaves a half-written
# package behind a valid sum. The toolchain is not in the sum, so the mount is
# keyed on the wasm-pack version: a bump starts from nothing.
FROM rust-src AS wasm
ARG WASM_PACK_VERSION
COPY scripts/build.mjs scripts/shared.mjs scripts/
RUN --mount=type=cache,target=/build/target,id=thunderforge-target \
  --mount=type=cache,target=/root/.cache,id=thunderforge-wasm-pack-cache \
  --mount=type=cache,target=/cache/dist,id=thunderforge-dist-wasm-pack-${WASM_PACK_VERSION} \
  mkdir -p dist && cp -a /cache/dist/. dist/ \
  && node scripts/build.mjs --only-wasm \
  && rm -rf /cache/dist/* && cp -a dist/. /cache/dist/

# --- the base maps -------------------------------------------------------------
#
# The example maps imported (spec 088): what a new world can open on, and
# what the demo's scenes are made of. The importer is a second binary of the
# server crate, built here in the dev profile — a map import does not need an
# optimised binary, and the release server with fat LTO is the longest step of
# the whole build. This stage shares nothing with it, so BuildKit runs the two
# side by side, and a landing build never waits on the server at all.
FROM toolchain AS base-maps-cook
WORKDIR /build
ENV CARGO_TERM_COLOR=never
COPY .cargo .cargo
COPY --from=planner /build/recipe.json recipe.json
RUN --mount=type=cache,target=/build/target,id=thunderforge-target \
  cargo chef cook --recipe-path recipe.json -p thunderforge

FROM base-maps-cook AS base-maps
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY packs packs
COPY config config
COPY legal legal
COPY examples/maps examples/maps
COPY apps/thunderforge apps/thunderforge
COPY apps/thunderforge-crucible-server apps/thunderforge-crucible-server
COPY apps/thunderforge-mapforge-server apps/thunderforge-mapforge-server
COPY apps/telemetry-gateway apps/telemetry-gateway
RUN --mount=type=cache,target=/build/target,id=thunderforge-target \
  cargo run -q -p thunderforge --bin thunderforge-demo-maps \
  --features demo-maps -- examples/maps /out/base-maps \
  && test -f /out/base-maps/NOTICE.txt

# --- the site: the demo, and whatever is built beside it ----------------------
#
# The whole tree now, for the pnpm workspace: `--frozen-lockfile` resolves
# every package the lockfile knows. The steps from here are seconds, so a
# layer that any edit invalidates costs nothing.
FROM wasm AS site
COPY . .
RUN --mount=type=cache,target=/root/.local/share/pnpm/store,id=thunderforge-pnpm-store \
  pnpm install --frozen-lockfile

# The demo (spec 074), written to data/demo: static files and nothing else.
# Its scenes are the example maps the base-maps stage imported.
COPY --from=base-maps /out/base-maps apps/demo/public/maps
RUN pnpm -F @thunderforge/demo run build

# --- the build ----------------------------------------------------------------
FROM site AS build
ARG BUILD_PROFILE

# The client bundle, written to data/client.
RUN pnpm -F @thunderforge/web run build

# The server in BUILD_PROFILE, stripped: a debug binary carries about 1.4GB
# of symbols and nothing in a container needs them.
RUN --mount=type=cache,target=/build/target,id=thunderforge-target \
  if [ "$BUILD_PROFILE" = dev ]; then flag=""; dir=debug; else flag=--release; dir=release; fi \
  && cargo build $flag -p thunderforge \
  && mkdir -p /out \
  && install -m 755 target/$dir/thunderforge /out/thunderforge \
  && strip /out/thunderforge \
  && install -m 755 /usr/local/cargo/bin/diesel /out/diesel \
  && strip /out/diesel

# --- the homepage ------------------------------------------------------------
#
# thunderforge.dev: apps/landing at / and the demo at /demo/, served by nginx.
# Build it with `--target landing`; the server stays the default (last) stage.
# It starts from `site`, not `build`: the homepage needs the demo and the wasm,
# never the server binary or the client bundle.
FROM site AS landing-build
# Then a .br beside every asset of both, and the engine's sizes in the demo's
# engine.json (spec 090 US4): nginx has no brotli module, so it sends these.
RUN pnpm -F @thunderforge/landing run build \
  && node apps/landing/scripts/compress.mjs

FROM nginx:stable-alpine AS landing
# github-auth.sh signs the GitHub App's JWT with openssl. With no App and an
# empty token the star chart shows contributors only.
RUN apk add --no-cache openssl
ENV GITHUB_TOKEN=""
COPY --chmod=755 apps/landing/github-auth.sh /docker-entrypoint.d/40-github-auth.sh
# Spec 086: browser telemetry to the project's endpoint, on by default.
# `TELEMETRY=false` serves `{"enabled":false}` and the pages load nothing for it.
ENV TELEMETRY=true \
  THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT=https://telemetry.thunderforge.dev \
  THUNDERFORGE_BROWSER_TELEMETRY_SAMPLE_RATE=1.0
COPY apps/landing/nginx.conf.template /etc/nginx/templates/default.conf.template
COPY --from=landing-build /build/apps/landing/dist /usr/share/nginx/html
COPY --from=landing-build /build/data/demo /usr/share/nginx/html/demo
EXPOSE 8080

# --- the public telemetry gateway (spec 086) ---------------------------------
#
# Its own image: the OTLP intake in front of the collector, published by
# `make push-telemetry-gateway`. Before `server`, so `server` stays the stage a
# plain `docker build` ends on. It cooks only its own dependencies, so a
# gateway image does not wait on the wasm crates.
FROM toolchain AS gateway-cook
WORKDIR /build
ENV CARGO_TERM_COLOR=never
COPY .cargo .cargo
COPY --from=planner /build/recipe.json recipe.json
RUN --mount=type=cache,target=/build/target,id=thunderforge-target \
  cargo chef cook --release --recipe-path recipe.json -p thunderforge-telemetry-gateway

FROM gateway-cook AS gateway-build
COPY . .
RUN --mount=type=cache,target=/build/target,id=thunderforge-target \
  cargo build --release -p thunderforge-telemetry-gateway \
  && install -D target/release/thunderforge-telemetry-gateway /out/thunderforge-telemetry-gateway \
  && strip /out/thunderforge-telemetry-gateway

FROM debian:bookworm-slim AS telemetry-gateway
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*
COPY --from=gateway-build /out/thunderforge-telemetry-gateway /usr/local/bin/
USER 65532:65532
EXPOSE 4318
ENTRYPOINT ["/usr/local/bin/thunderforge-telemetry-gateway"]

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

# The maps a new world can open on (spec 088): the same import the demo's
# scenes come from, with its credit and NOTICE.txt. Not under the data
# directory, for the client's reason: an image rebuild replaces them.
COPY --from=base-maps /out/base-maps /srv/base-maps

COPY scripts/container-entrypoint.sh /usr/local/bin/thunderforge-entrypoint
# Belt and braces: COPY carries the host's mode, and a script that arrives
# without the execute bit fails at `docker run`, not at `docker build`.
RUN chmod 0755 /usr/local/bin/thunderforge-entrypoint

ENV THUNDERFORGE_DATA_PATH=/srv/thunderforge/data \
  STATIC_DIR=/srv/thunderforge/client \
  DEMO_DIR=/srv/thunderforge/demo \
  THUNDERFORGE_BASE_MAPS_DIR=/srv/base-maps
# Spec 086: the binary's default, written down so `docker inspect` shows it.
ENV TELEMETRY=true
EXPOSE 30000

ENTRYPOINT ["/usr/local/bin/thunderforge-entrypoint"]
