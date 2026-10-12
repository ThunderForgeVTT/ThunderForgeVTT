#!/usr/bin/env bash
#
# The CI pipeline, run through paws (github.com/mbround18/paws) so a laptop
# and GitHub Actions run the same thing: `make ci` here, `.github/workflows/
# ci.yaml` there.
#
# What it runs is what needs no cluster and no database: every check in
# `scripts/verify.mjs` (the pre-push hook's eighteen), `make lint` (clippy
# with -D warnings for the host and for wasm32), the web typecheck and the web
# unit tests. Not here, and why:
#
# - `cargo test`: the server crate's tests need Postgres and RustFS, and
#   `paws run` has no service containers. They stay `make test-rust`.
# - e2e: proved per change with `pnpm e2e:<slice>`, locally.
#
# The image mirrors the Dockerfile's `toolchain` stage (same Rust, Node, pnpm
# and wasm-pack versions), so a red check here is red in the image build too.
# Keep the versions below in step with the Dockerfile's ARGs.
set -euo pipefail

RUST_VERSION=1.98
NODE_VERSION=24.19.0
PNPM_VERSION=10.33.2
WASM_PACK_VERSION=0.15.0

if ! command -v paws >/dev/null 2>&1; then
  echo "paws is not installed: curl -fsSL https://raw.githubusercontent.com/mbround18/paws/main/scripts/install.sh | sh" >&2
  exit 127
fi

cd "$(dirname "$0")/.."

# `paws run` leaves `.git` out of the upload. Several checks read
# `git ls-files`, so the first step indexes what was sent: the tree filtered
# by `.gitignore`, which is what a fresh checkout holds.
exec paws run \
  --image "rust:${RUST_VERSION}-bookworm" \
  --apt ca-certificates,clang,curl,git,libpq-dev,pkg-config,xz-utils,make \
  --setup 'arch=$(dpkg --print-architecture | sed s/amd64/x64/) && curl -fsSL "https://nodejs.org/dist/v'"${NODE_VERSION}"'/node-v'"${NODE_VERSION}"'-linux-${arch}.tar.xz" | tar -xJ -C /usr/local --strip-components=1' \
  --setup "npm install --global pnpm@${PNPM_VERSION}" \
  --setup "rustup target add wasm32-unknown-unknown && rustup component add clippy rustfmt" \
  --setup "cargo install wasm-pack --version ${WASM_PACK_VERSION} --locked" \
  --env CARGO_BUILD_JOBS \
  --env CARGO_TERM_COLOR=always \
  --cache tf-ci-target=target \
  --cache tf-ci-cargo-registry=/usr/local/cargo/registry \
  --cache tf-ci-pnpm-store=/root/.local/share/pnpm/store \
  --cache tf-ci-wasm-pack=/root/.cache \
  --step 'git init -q && git add -A' \
  --step 'ENGINE_PROFILE=dev node scripts/build.mjs --only-wasm' \
  --step 'pnpm install --frozen-lockfile' \
  --step 'node scripts/verify.mjs' \
  --step 'make lint' \
  --step 'pnpm -F @thunderforge/web run typecheck' \
  --step 'pnpm -F @thunderforge/web run test'
