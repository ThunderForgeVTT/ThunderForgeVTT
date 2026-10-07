# Implementation Plan: Resumable Downloads

**Branch**: `main` | **Date**: 2026-10-07 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/080-resumable-downloads/spec.md`

## Summary

Large files stop restarting from zero. The server learns to send part of a
stored file, and the web app gets one downloader that fetches a large file in
a few parts at once, resumes a part that broke, and hands the bytes on in
order as one stream.

- **Asset routes** (canvas, scene, lore, actor) read a byte range from
  storage as they send it, answer `206` with `Accept-Ranges`, `ETag` and
  `Content-Range`, answer `416` with the real size, and honour `If-Range`.
  Permission is checked exactly as today, on every request
  ([research R2](./research.md#r2-reading-a-range-from-storage)).
- **Built files** are compressed once at build time (`.br`, `.gz`) and
  `ServeDir` serves those copies, so the on-the-fly compression layer no
  longer strips `Accept-Ranges` from them (R4).
- **`packages/downloads`** is the downloader. A plain first request; parts
  only for a large, uncompressed answer that offers ranges and a validator;
  every part checked against the first answer's version; bytes out in file
  order with memory bounded by the parts in flight (R5, R6).
- **Callers**: the engine loader, scene preload and the engine's world cache
  all go through it (R8, R9). A runtime flag turns parts off (R7).

What is deliberately not built: decompressing a precompressed file in the
page, which would only matter if the release engine grew past 16 MB after
compression (R5).

## Technical Context

**Language/Version**: Rust 2021 (server, unchanged edition); TypeScript 5
(the new package and the web app); Rust → `wasm32-unknown-unknown` (engine,
one extern added).

**Primary Dependencies**: `axum` 0.8, `tower-http` 0.7 (`ServeDir`
precompressed, `CompressionLayer` predicate), `aws-sdk-s3` 1.144
(`GetObject.range`, `.if_match`, `HeadObject`), `tokio-util` (`ReaderStream`,
added if not already reachable). Web: no new runtime dependency; Vite's
`closeBundle` hook with `node:zlib`.

**Storage**: RustFS through the existing scoped-credential path. No schema
change, no migration.

**Testing**: `cargo test -p thunderforge-server` (ranged reads against the
real test bucket, `thunderforge_test`; see memory: the RustFS bucket must
exist); `node --test` in `packages/downloads`; Vitest for the web callers;
Playwright slice `resumable-downloads`.

**Target Platform**: Linux server; Chromium/Firefox/WebKit; engine on wasm32.

**Project Type**: web service + web app + shared web library.

**Performance Goals**: SC-002 (≥1.5× on a per-connection-throttled link),
SC-003 (first engine load no slower), SC-004 (return visit zero bytes),
SC-007 (server memory per request bounded by the part, not the file).

**Constraints**: the downloader holds at most `concurrency × partSize`
(32 MiB by default) beyond what the caller keeps (FR-013). No React, engine
or app import in `packages/downloads` (FR-011). The engine's fingerprint
check stays exactly as it is (FR-022).

**Scale/Scope**: four asset routes, three static mounts, one new package,
three callers, one flag, one slice.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. ECS owns simulation, React owns chrome** — pass. No component gains
  network code; the engine gains no network task (it calls one page
  function through an extern, R8, as it already calls `gloo_net`).
- **II. Plugin-modular engine** — pass. The change stays inside
  `cached_assets/wasm.rs::fetch`.
- **III. Ownership and authorization at the data boundary** — pass, and it
  is the point of US4. Each route keeps its existing check before any
  storage call; the ranged read is minted the same one-object scoped
  credential (FR-005). Server tests refuse a part on every route.
- **IV. ADRs and specs before divergence** — pass. No ADR is contradicted:
  storage stays private (ADR-039), and no pre-signed URL is introduced.
- **V. Verify before claiming done** — the server is checked with
  `cargo check`/`clippy` on the host; the engine change with
  `cargo check --target wasm32-unknown-unknown -p thunderforge-engine`
  (memory: lint has two targets). The package with `tsc --noEmit`.
- **VI. Every feature is proven by its own slice** —
  - **Slice**: `resumable-downloads`, run as `pnpm e2e:resumable-downloads`.
  - **Own specs**: `resumable-downloads-*.spec.ts` (prefix).
  - **Standalone half**: yes. `e2e:resumable-downloads:standalone` runs
    `pnpm -F @thunderforge/downloads test:e2e`: the real package against a
    real Node HTTP server that serves ranges, drops a connection mid-part
    and swaps the file mid-download, with no stack (R10).
  - **Neighbours, by the seam each crosses**:
    - `engine-loading.spec.ts` (owned by `engine-other`): the engine loader
      and its "Waking the engine" progress now go through the downloader.
    - `scene-preload.spec.ts` (owned by `world-cache`): preload now warms
      through the downloader.
    - `world-cache-prefetch.spec.ts` if present, else the `world-cache`
      spec that fetches a canvas asset into the cache: the engine's world
      cache fetch now goes through the bridge.
    - `canvas-asset-access.spec.ts` or the nearest spec that is refused an
      asset it may not see: the asset routes' answers changed.
    - (Chosen by `pnpm e2e:which --diff` once the code lands; the slice
      entry records the final names.)
  - **Cross-cutting?** Partly. `apps/thunderforge/src/main.rs`'s compression
    predicate and `static_files` touch every page load. Every slice loads
    built files, so a broken static mount would fail any slice; the slice
    plus `engine-loading` covers it, and the full suite runs before the
    owner deploys.

No violations; Complexity Tracking is empty.

**Post-design re-check**: unchanged. The design added no dependency beyond
`tokio-util` (already in the lock through `tower-http`), no new service and
no new storage.

## Project Structure

### Documentation (this feature)

```text
specs/080-resumable-downloads/
├── plan.md              # This file
├── research.md          # Phase 0: R1–R10
├── data-model.md        # Version tag, Part, Download, settings
├── quickstart.md        # How to see and prove it
├── contracts/
│   ├── http-ranges.md   # What every asset route and built file answers
│   └── downloader.md    # @thunderforge/downloads API
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
crates/thunderforge-server/src/
├── storage/rustfs.rs            # + open_object(): ranged, streaming GetObject; head size
├── assets_serve/
│   ├── ranged.rs                # new: parse Range/If-Range, build 200/206/416 from an open object
│   ├── ranged_tests.rs          # new: header parsing and answer shapes
│   ├── canvas.rs scene.rs lore.rs actor.rs   # read through ranged::serve
│   └── *_tests.rs               # + a part answer and a refused part per route
└── static_files/mod.rs          # ServeDir .precompressed_br().precompressed_gzip()

apps/thunderforge/src/main.rs    # compression predicate leaves video/audio alone

packages/downloads/              # new: @thunderforge/downloads
├── package.json                 # node --test, tsc --noEmit, test:e2e
├── src/{index,settings,download,parts,errors}.ts
├── src/*.test.ts                # fake-fetch unit tests
└── e2e/                         # real Node server + real package (standalone half)

apps/web/
├── vite.config.mts              # precompress plugin (closeBundle)
├── src/engine/bevy/index.ts     # engine fetch through the downloader; installs the bridge
├── src/services/scenePreload.ts # preload through the downloader
├── src/services/downloads.ts    # new: app wiring (settings from flag + DEV override)
├── src/api/featureFlags.ts      # FEATURE_DOWNLOAD_IN_PARTS
└── e2e/resumable-downloads-*.spec.ts

crates/thunderforge-engine/src/plugins/cached_assets/wasm.rs  # fetch via bridge, gloo fallback
crates/thunderforge-server/src/settings/{features.rs,registry/declarations.rs}  # the flag

scripts/e2e/slices.json          # + resumable-downloads (standalone, own, neighbours, paths)
package.json                     # + e2e:resumable-downloads{,:standalone,:integration}
pnpm-workspace.yaml              # already globs packages/*
```

**Structure Decision**: the downloader is a library in `packages/`, the
app's wiring is a thin service in `apps/web`, and the server's range logic
is one module beside the routes that use it. This follows the owner's
apps-are-thin rule and the constitution's layout rule.

**Slice**: `scripts/e2e/slices.json` gains `resumable-downloads` with a
`standalone` command, `own: ["resumable-downloads-"]`, the neighbours above
with their seams, and `paths` for every file in the tree above. The root
`package.json` gains the three scripts (generated shape checked by
`scripts/check-e2e-slices.mjs`).

## Complexity Tracking

None.
