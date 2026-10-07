# Tasks: Resumable Downloads

**Input**: [spec.md](./spec.md), [plan.md](./plan.md), [research.md](./research.md),
[data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: requested by the spec's Proof section: server tests, downloader
unit tests, and the `resumable-downloads` slice. Tests come before the code
they prove inside each phase.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an open task)
- **[Story]**: US1 dropped connection resumes · US2 engine wakes in parts ·
  US3 never spliced · US4 permission on every part · US5 small files stay simple

---

## Phase 1: Setup

- [x] T001 Create `packages/downloads/package.json` (`@thunderforge/downloads`, `private`, `type: module`, exports `./src/index.ts`, scripts `test: node --test src/`, `test:e2e: node --test e2e/`, `typecheck: tsc --noEmit`, license AGPL-3.0-or-later, copied from `packages/hero-builder/package.json`) and `packages/downloads/tsconfig.json`; add the package to `apps/web/package.json` dependencies as `workspace:*`; run `pnpm install`
- [x] T002 [P] Confirm `tokio-util` (feature `io`) is a direct dependency of `crates/thunderforge-server/Cargo.toml`; add it at the version already in `Cargo.lock` if not — not needed: the body streams from `ByteStream` directly

---

## Phase 2: Foundational (blocks every story)

### The server reads ranges from storage

- [x] T003 Write `crates/thunderforge-server/src/assets_serve/ranged_tests.rs`: header parsing (`bytes=a-b`, `a-`, `-n` accepted; comma-separated → whole; garbage → whole; `If-Range` etag vs date) and answer shapes (200 has `Accept-Ranges`+`ETag`+`Content-Length`; 206 has `Content-Range`; 416 has `bytes */size` and an empty body)
- [x] T004 Add `open_object(cfg, key, range: Option<&str>, if_match: Option<&str>) -> Result<OpenObject, StorageError>` and `object_size(cfg, key)` to `crates/thunderforge-server/src/storage/rustfs.rs`: scoped credential as `read_object`; `GetObject.range().if_match()`; body as a `Stream` via `into_async_read` + `ReaderStream`; carries `content_length`, `content_range`, `e_tag`, `last_modified`; new `StorageError` variants `RangeNotSatisfiable` and `PreconditionFailed`
- [x] T005 Implement `crates/thunderforge-server/src/assets_serve/ranged.rs`: `pub async fn serve(cfg, key, headers: &HeaderMap, extra: &[(HeaderName, &'static str)]) -> Response` per [contracts/http-ranges.md](./contracts/http-ranges.md) (412 on If-Range → whole file; InvalidRange → `object_size` → 416); register it and its tests in `assets_serve/mod.rs`; T003 passes

### The downloader core

- [x] T006 [P] Write `packages/downloads/src/settings.ts` (`DownloadSettings`, `DEFAULT_DOWNLOAD_SETTINGS`, `resolveSettings(partial)`) and `src/errors.ts` (`DownloadError`, `VersionChangedError`)
- [x] T007 [P] Write a scripted fake fetch for tests in `packages/downloads/src/testing/fakeServer.ts`: serves a byte array with ranges, ETag, optional `Content-Encoding`, and per-request hooks to delay, cut after N bytes, change the version or refuse
- [x] T008 Write `packages/downloads/src/download.test.ts`: in-order output when parts finish out of order; part 0 is the first answer's body and no byte is fetched twice on a clean run; memory bound (never more than `concurrency` parts started ahead of the delivery point); progress non-decreasing and ends at `total`; cancellation aborts every request in flight
- [x] T009 Implement `packages/downloads/src/download.ts` and `src/parts.ts` (`download()`, `downloadBytes()`, `Download.toResponse()`) per [contracts/downloader.md](./contracts/downloader.md) and research R6; export from `src/index.ts`; T008 passes; `tsc --noEmit` clean

### App wiring

- [x] T010 Declare `feature.download_in_parts` (default on, public to members) in `crates/thunderforge-server/src/settings/registry/declarations.rs` and the `FEATURES` list in `settings/features.rs`; add `FEATURE_DOWNLOAD_IN_PARTS` in `apps/web/src/api/featureFlags.ts`
- [x] T011 Write `apps/web/src/services/downloads.ts`: `downloadSettings()` (flag off → `enabled: false`; reads as on before flags answer; merges `globalThis.__thunderforgeDownloadSettings` only under `import.meta.env.DEV`) and thin `download`/`downloadBytes` wrappers that pass it; Vitest in `apps/web/src/services/__tests__/downloads.test.ts`

**Checkpoint**: server answers ranges for a key; the package passes alone.

---

## Phase 3: US1 — A dropped connection does not restart a scene (P1) 🎯 MVP

**Independent test**: a large background whose connection is cut halfway completes, and the bytes fetched after the cut are only the missing ones.

- [x] T012 [P] [US1] In `packages/downloads/src/download.test.ts`: a part cut partway is resumed from its `received` offset alone, with a growing pause; parts already received are kept; past `retries` the stream errors with `DownloadError` and nothing is delivered as whole
- [x] T013 [US1] Implement resume-inside-a-part and retry backoff in `packages/downloads/src/parts.ts`; T012 passes
- [x] T014 [US1] Serve `canvas.rs`, `scene.rs`, `lore.rs`, `actor.rs` in `crates/thunderforge-server/src/assets_serve/` through `ranged::serve`, keeping each route's `Content-Type` and `Cache-Control` and its authorization exactly as is; add a 206 test to `canvas.rs`'s tests
- [x] T015 [US1] `apps/web/src/services/scenePreload.ts`: warm through `downloadBytes(url, { init: { cache: "force-cache" } })`, keep `{warmed, reason}`; update its Vitest
- [x] T016 [US1] Engine world cache: install `globalThis.__thunderforgeDownloadBytes` in `apps/web/src/engine/bevy/index.ts` before the engine starts; in `crates/thunderforge-engine/src/plugins/cached_assets/wasm.rs` `fetch()` call it through a `wasm_bindgen` extern when defined, fall back to `gloo_net`; fingerprint check untouched; `cargo check --target wasm32-unknown-unknown -p thunderforge-engine`
- [x] T017 [US1] Standalone e2e `packages/downloads/e2e/resume.test.ts`: a real `node:http` server serving a 40 MB generated file with ranges and ETag, which destroys the socket of one part halfway; assert the result's SHA-256 matches and the bytes served after the cut ≤ the parts in flight (SC-001), using real default settings
- [x] T018 [US1] Integration e2e `apps/web/e2e/resumable-downloads-scene.spec.ts`: lower the threshold via the DEV override, open a scene whose background goes into parts, abort one ranged request with `page.route` partway, assert it is re-requested with a later start offset and the board renders the background

---

## Phase 4: US2 — The engine wakes in parts (P1)

**Independent test**: a cold board load fetches the development engine as several `206` parts and the "Waking the engine" bar only moves forward; a reload downloads zero engine bytes.

- [x] T019 [P] [US2] Precompress plugin in `apps/web/vite.config.mts` (`closeBundle`, `node:zlib` brotli q11 + gzip 9 for `.js .css .wasm .json .svg .map` under `assets/{entry,chunks,static}`); verify `pnpm -F web build` writes the copies — above 64 MB (a dev-profile engine) brotli drops to quality 9: q11 took 4 min on 271 MB
- [x] T020 [P] [US2] `crates/thunderforge-server/src/static_files/mod.rs`: `.precompressed_br().precompressed_gzip()` on the three built mounts; test that a `br` request gets `Content-Encoding: br` + `Accept-Ranges` + `ETag` and an identity `Range` request gets `206` of the original — `ServeDir` names its version by `Last-Modified`, not `ETag`; the downloader accepts either
- [x] T021 [P] [US2] `apps/thunderforge/src/main.rs`: compression predicate also skips `video/*` and `audio/*`
- [x] T022 [US2] `apps/web/src/engine/bevy/index.ts`: `fetchWasmWithProgress` uses `download()` and hands `toResponse()` to `wasm.default({ module_or_path })`, mapping `onProgress` to the existing `{stage: "downloading", loaded, total}`; return visits keep the no-delay rule
- [ ] T023 [US2] Integration e2e `apps/web/e2e/resumable-downloads-engine.spec.ts`: on a cold context, record the engine requests; assert several `206` answers with distinct ranges, progress reports non-decreasing, and the engine reaches ready

---

## Phase 5: US3 — A file changed on the server is never spliced (P1)

**Independent test**: a file replaced mid-download is delivered whole as the new version, or the download fails; never a mix.

- [x] T024 [P] [US3] Server test in `ranged_tests.rs` (needs storage): `If-Range` with a stale ETag answers `200` with the whole current object; matching ETag answers `206`
- [x] T025 [P] [US3] Downloader tests: a part answered `200`, `416`, or with another ETag before any byte is delivered restarts once and yields the new version; after bytes were delivered the stream errors with `VersionChangedError`; `downloadBytes` restarts and returns the new version
- [x] T026 [US3] Implement the per-part validator check and the restart rule in `packages/downloads/src/parts.ts`; T025 passes
- [x] T027 [US3] Standalone e2e `packages/downloads/e2e/version.test.ts`: the real server swaps the file between parts; the result hashes to one version, run 20 times (SC-005)

---

## Phase 6: US4 — Permission is checked on every part (P1)

**Independent test**: a caller refused the whole file is refused every range of it, with no size leaked.

- [x] T028 [US4] Tests in each of `assets_serve/{canvas,scene,lore,actor}.rs` test modules: a non-member sending `Range: bytes=0-9` gets the route's existing 403/404 with no `Content-Range`, no `ETag` and no file-sized `Content-Length` (SC-006) — lore checked with an unknown id: its permission ladder admits a non-member as Viewer, a gap that predates this spec
- [x] T029 [US4] Downloader test: a `403` first answer rejects with `DownloadError(status 403)` and is not retried

---

## Phase 7: US5 — Small files stay simple (P2)

**Independent test**: below the threshold, without ranges, without a validator, with `Content-Encoding`, or with the flag off, a download is exactly one plain request.

- [x] T030 [US5] Downloader tests for each fallback case (one request, no `Range` header sent, body passed through)
- [ ] T031 [US5] Integration e2e step in `resumable-downloads-engine.spec.ts`: with default settings a small asset makes one request with no `Range`

---

## Phase 8: Proof and polish

- [x] T032 Register the slice: `scripts/e2e/slices.json` entry `resumable-downloads` (`standalone: "pnpm -F @thunderforge/downloads test:e2e"`, `own: ["resumable-downloads-"]`, neighbours `engine-loading.spec.ts` and `scene-preload.spec.ts` with seams, `paths` for every file touched); root `package.json` scripts; `node scripts/check-e2e-slices.mjs` passes
- [ ] T033 `cargo fmt`, `cargo clippy -p thunderforge-server`, `cargo check --target wasm32-unknown-unknown -p thunderforge-engine`, `pnpm -F @thunderforge/downloads typecheck`, `pnpm -F web typecheck`
- [x] T034 `cargo test -p thunderforge-server assets_serve static_files` green
- [ ] T035 Run `pnpm e2e:resumable-downloads`; record the result here
- [x] T036 [P] User guide note in `docs/guides/` on the `feature.download_in_parts` switch; CONTRIBUTING note on the package and the DEV override

---

## Dependencies

- Phase 1 → Phase 2 → stories. US1 needs T005 and T009. US2 needs T009/T011 only (built files do not use `ranged.rs`). US3 and US4 extend US1's code. US5 is tests on T009.
- Order of delivery: US1 (MVP) → US3 → US4 → US2 → US5 → Phase 8.

## Parallel opportunities

- T002 beside T001; T006/T007 beside T003–T005 (Rust vs TypeScript).
- T019, T020, T021 are separate files.
- T024 and T025 in parallel; T028's four route modules in parallel.

## Implementation strategy

MVP is US1: ranged routes plus a downloader that resumes, used by scene preload and the world cache. US3 and US4 harden it before any release. US2 brings the engine in.
