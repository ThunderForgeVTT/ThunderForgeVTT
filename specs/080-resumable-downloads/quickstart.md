# Quickstart: Resumable Downloads

## Prove it

```bash
# Server: ranges, If-Range, 416, refusal per route (needs Postgres + RustFS bucket)
cargo test -p thunderforge-server assets_serve

# Downloader unit tests (no browser, no server)
pnpm -F @thunderforge/downloads test

# The slice: standalone half (real Node server, no stack), then integration
pnpm e2e:resumable-downloads
```

## See it by hand

With the stack up (`pnpm dev`), signed in, on a scene with a background:

```bash
# A part, then the version check
curl -s -o /dev/null -D - -H 'Range: bytes=0-99' -b cookies.txt \
  http://localhost:5173/api/canvas-assets/<id>.webp     # 206, Content-Range: bytes 0-99/<size>
curl -s -o /dev/null -D - -H 'Range: bytes=0-99' -H 'If-Range: "nope"' -b cookies.txt \
  http://localhost:5173/api/canvas-assets/<id>.webp     # 200, whole file
curl -s -o /dev/null -D - -H 'Range: bytes=999999999-' -b cookies.txt \
  http://localhost:5173/api/canvas-assets/<id>.webp     # 416, Content-Range: bytes */<size>
```

In the browser, DevTools → Network on a cold load of the board:

- **Development engine** (~271 MB): several `206` requests for
  `engine_bg*.wasm`, four in flight, and the "Waking the engine" bar moving
  forward only.
- **Release build** (`pnpm build`, served by the binary): one
  `engine_bg*.wasm` request with `Content-Encoding: br`, and on reload
  zero bytes (from the cache).

Turn parts off with the `feature.download_in_parts` flag in Admin →
Settings → Features; every download is then a single request again.
