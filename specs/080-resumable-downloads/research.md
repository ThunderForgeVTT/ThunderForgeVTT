# Research: Resumable Downloads

Read on 2026-10-07 against `main` at `a28a819e`. Each entry is a decision,
why, and what else was weighed.

## R1. What the server already does with ranges

**Finding.** Two different serving paths, two different answers.

- **Built files** go through `tower_http::services::ServeDir` (tower-http
  0.7.0, `static_files/mod.rs`). It already answers a single `Range` with
  `206` and `Content-Range`, refuses a bad one with `416` and
  `bytes */size`, refuses several ranges with `416`, and sends
  `Accept-Ranges: bytes`, a strong `ETag` (size + mtime) and
  `Last-Modified`. It honours `If-Match` and `If-None-Match`. It does
  **not** read `If-Range`. It serves precompressed siblings (`.br`, `.gz`)
  when `.precompressed_br()` / `.precompressed_gzip()` are switched on,
  which they are not today.
- **Asset routes** (`assets_serve/{canvas,scene,lore,actor}.rs`) call
  `storage::rustfs::read_object`, which `collect()`s the whole object into
  a `Vec<u8>` and answers `200` with no `Accept-Ranges` and no `ETag`.

**The compression layer** (`apps/thunderforge/src/main.rs:775`,
`CompressionLayer::new().br(true).gzip(true)`, default predicate) skips a
response that already has `Content-Encoding` or `Content-Range`, and
skips `image/*`. When it *does* compress a `200`, it **removes
`Accept-Ranges`**. So today a compressible built file never advertises
ranges to a browser that accepts compression, and a video or audio asset
would lose it too.

**Decision.** Built files keep `ServeDir`; asset routes get a shared ranged
reader. The compression layer is told not to touch `video/*` and
`audio/*`, which it cannot usefully shrink anyway.

## R2. Reading a range from storage

**Decision.** A new `storage::rustfs::open_object(cfg, key, range,
if_range)` sends `GetObject` with `.range(..)` and returns the body as a
stream (`ByteStream::into_async_read` wrapped in
`tokio_util::io::ReaderStream`, handed to `axum::body::Body::from_stream`),
together with `Content-Length`, `Content-Range`, `ETag` and `Last-Modified`
from the storage answer. The scoped one-object credential is minted per
request exactly as `read_object` mints it now (FR-005: a part is a
request like any other).

- **The range string** is passed to storage as the client sent it, once
  the server has checked it is a single `bytes=` range (`a-b`, `a-` or
  `-n`). A header with a comma is several ranges, and is answered with the
  whole file (FR-007). A header that does not parse is ignored, which is
  what HTTP says a server should do.
- **416.** Storage refuses a range past the end with `InvalidRange`. The
  server then asks `HeadObject` for the size and answers `416` with
  `Content-Range: bytes */size` (FR-004). `HeadObject` needs the same
  `s3:GetObject` permission the scoped policy already grants.
- **If-Range** (FR-003). When the request carries `If-Range` with an
  entity tag, the server sends `GetObject` with `.range(..)` **and**
  `.if_match(tag)`. Storage answers `412` when the object has changed; the
  server then asks again with no range and answers `200` with the whole
  current file. The extra storage call happens only when a version really
  changed. An `If-Range` carrying a date is treated as "do not trust the
  range": the whole file.
- **Whole reads** take the same path with no range, so whole requests stop
  buffering too (FR-006's SHOULD). `read_object` stays for the callers that
  need bytes in memory (transcoding, thumbnails, exports).
- **ETag.** RustFS returns the S3 `ETag` for an object (MD5 for a single
  upload, quoted). It changes when the content changes, which is all
  FR-002 asks. The routes pass it through untouched.

**Alternatives.** Buffering the object and slicing it in memory (rejected:
it is the problem FR-006 exists to remove). Pre-signed storage URLs
(rejected: ADR-039 keeps storage private, and spec 028 T045c closed the one
side door this would reopen).

**Feedback attachments** (`assets_serve/feedback.rs`) are `no-store`
attachments from a form, not table content, and are left as they are.

## R3. If-Range on built files

`ServeDir` ignores `If-Range`. Every built file is named by the hash of its
bytes (`engine_bg-D9SKK7e3.wasm`, `static_files/mod.rs`'s "kept until its
name changes"), so a given name never changes version.

**Decision.** No server work. The downloader checks the validator on
*every* part answer (R6), so a server that ignores `If-Range` and sends a
part of a changed file is caught on the client. That check is what makes
FR-015 hold against any server, `ServeDir` included.

## R4. Precompressed built files

**Decision.** A small Vite plugin in `apps/web/vite.config.mts`
(`closeBundle`) writes `.br` (quality 11) and `.gz` (level 9) beside every
compressible file under `assets/{entry,chunks,static}` (`.js`, `.css`,
`.wasm`, `.json`, `.svg`, `.map`), using `node:zlib` and no new dependency.
The three built mounts turn on `.precompressed_br().precompressed_gzip()`.
A precompressed answer carries `Content-Encoding`, so the compression layer
leaves it alone, and `ServeDir` keeps `Accept-Ranges` and `ETag` on it. A
browser that accepts neither encoding gets the original file (FR-010).

The same build runs in Docker (`pnpm` web stage), so the image ships the
compressed copies with no Dockerfile change.

## R5. Ranges and Content-Encoding do not mix in a browser

The Fetch standard has a browser send `Accept-Encoding: identity` on any
request with a `Range` header. A server that compresses therefore sends a
range of the **uncompressed** file, while the plain `200` it sent first
counted the **compressed** size. Mixing the two would splice garbage.

**Decision.** The downloader starts with a plain request (R6). If that
answer has a `Content-Encoding` other than `identity`, the file is
delivered from that single answer and no part is ever asked for. In
practice:

- The **release engine** is about 4–5 MB as brotli, well under the 16 MB
  threshold, and arrives in one compressed request exactly as today, kept
  by the browser's cache exactly as today (SC-003, SC-004).
- The **development engine** (271 MB) is served by the Vite dev server
  with no compression and with ranges, so it is the engine build that
  arrives in parts, in development and in the e2e harness. That is the
  path the slice proves.
- **Scene assets** are images and are not compressed, so they get parts
  whenever they are over the threshold.

**Not done, recorded for later.** A release engine that grows past the
threshold *after* compression would need its compressed bytes served as
an opaque file and decompressed in the page. That is not built here: it is
not needed at 4–5 MB, and building it now would mean shipping a
decompressor path no build exercises.

## R6. The downloader's shape

**Decision.** A new web library, `packages/downloads`
(`@thunderforge/downloads`), with no React, no engine and no app import
(FR-011, constitution layout rule).

1. **First request is plain**, as today: `GET` with no `Range`. Small files
   end here in one request (FR-016), and the browser caches them as
   before.
2. If that answer is `200`, has a known `Content-Length` at or over the
   threshold, says `Accept-Ranges: bytes`, has a validator (`ETag`,
   preferring a strong one, else `Last-Modified`) and no
   `Content-Encoding`, the file goes into parts. The first answer keeps
   streaming as **part 0** and is cancelled once it has delivered
   `partSize` bytes; parts 1..n are asked for with
   `Range: bytes=a-b` and `If-Range: <validator>`, a few at a time.
3. **Every part answer is checked.** `206` with a `Content-Range` that
   matches the asked range and the same validator is accepted. `200`
   (the server ignored the range or the version changed) or a different
   validator is a version change. `416` is a version change (the file
   shrank).
4. **Resume inside a part.** A part that fails partway is asked again for
   only the bytes it still lacks (`Range: bytes=received-b`), after a
   growing pause, up to the retry limit. Part 0 resumes the same way.
5. **In-order stream.** Bytes leave the downloader in file order as one
   `ReadableStream<Uint8Array>`. A part that finishes early waits in memory
   until the parts before it have been handed on. The downloader never
   starts more parts than `concurrency` ahead of the next byte it owes, so
   what it holds is bounded by `concurrency × partSize`, not by the file
   (FR-013).
6. **Version change.** If no byte has been handed on yet, the download
   starts over against the new version by itself, once. If bytes have
   already gone downstream, they cannot be taken back, so the stream fails
   with a `VersionChangedError` and the caller starts again. For
   `downloadBytes()` (whole file in memory) the restart is internal.
7. **Progress** counts bytes received on the wire per part offset, so a
   retried range is never counted twice and the count never goes down. An
   unknown total is reported as `null` (FR-017, spec 028 FR-030).
8. **Cancellation** is an `AbortSignal`; aborting stops every part in
   flight (FR-018).
9. **Failure** after the retry limit is one `DownloadError` naming the URL
   and the last status; the stream errors, so nothing partial is ever
   delivered as whole (FR-019).

**Alternatives.** A service worker that turns any fetch into parts
(rejected: `public/sw.js` is a cache janitor only, and a worker that
rewrites every fetch is a much larger change with its own lifecycle bugs).
`Range`-first probing (rejected: the `Range` header forces identity
encoding, so a small compressed file like the release engine would arrive
uncompressed, five times larger, and bypass the cache it hits today).

## R7. Settings and the switch

**Decision.**

- One `DownloadSettings` object with the defaults in the package:
  `threshold` 16 MiB, `partSize` 8 MiB, `concurrency` 4, `retries` 5,
  `retryDelayMs` 250 doubling (FR-023). Callers pass overrides; no caller
  restates a default.
- A runtime feature flag, `feature.download_in_parts` (spec 068), declared
  on by default, members only. Off means every download is one plain
  request, as before this spec. It reads as on until the flags have been
  answered, because "on" is the declared default; a first engine load must
  not wait on a flags round-trip.
- A development-only override for the e2e harness:
  `window.__thunderforgeDownloadSettings`, honoured only when
  `import.meta.env.DEV` is true (a compile-time switch, as the owner's
  rules allow for local development). The harness sets a small threshold
  so ordinary test assets go into parts.

## R8. The engine's world cache

`cached_assets/wasm.rs::fetch` uses `gloo_net` and keeps the whole file as
a `Vec<u8>` before fingerprint verification, which is unchanged.

**Decision.** The engine loader installs one function on the page,
`globalThis.__thunderforgeDownloadBytes(url): Promise<Uint8Array>`, built
on `downloadBytes()`, before the engine starts. `fetch` in `wasm.rs` calls
it through a `wasm_bindgen` extern when it is present and falls back to
`gloo_net` when it is not (the demo, a page that never installed it).
One downloader serves both sides (FR-011, FR-022), and the engine still
has no network code of its own beyond what it had.

## R9. Scene preload

`scenePreload.ts` warms the browser cache with one `force-cache` fetch.

**Decision.** It calls `downloadBytes(url, { cache: "force-cache" })`. A
background under the threshold is unchanged: one request, cached. A
background over it gains retries and resume. Whether the browser can later
serve a whole-file request from the parts it kept is the browser's
business; the engine's world cache, which is where a scene open actually
reads from, is filled by the same downloader on open. The slice records
what the browser did rather than claiming it.

## R10. Proof without a giant fixture

A 16 MB+ map is a slow fixture. **Decision**: the standalone half drives
the real package against a real Node HTTP server that serves ranges, cuts
a connection mid-part, and swaps the file mid-download, with the real
defaults and a 40 MB generated file. The integration half lowers the
threshold through the R7 override so an ordinary uploaded background goes
into parts through the real server, real storage and real permission
checks, and the development engine goes into parts as it is.
