# Contract: HTTP ranges

## Asset routes

`GET /api/canvas-assets/{id}[.ext]`, `GET /api/scene-assets/{id}/thumb`,
`GET /api/lore-assets/{id}[/thumb]`, `GET /api/actor-assets/{id}[.ext|/thumb]`.
Each route's own headers (`Content-Type`, `Cache-Control`) are unchanged.

Permission is decided **first**, exactly as today. A refused caller gets the
route's existing `403`/`404` with its existing text body: no `Content-Range`,
no `Content-Length` of the file, no `ETag` (FR-005, SC-006).

| Request | Answer |
| --- | --- |
| no `Range` | `200`, whole body streamed, `Accept-Ranges: bytes`, `ETag`, `Content-Length` |
| `Range: bytes=a-b` / `a-` / `-n`, inside the file | `206`, `Content-Range: bytes a-b/size`, `Content-Length: b-a+1`, `Accept-Ranges`, `ETag` |
| as above + `If-Range: "<etag>"` matching | `206` as above |
| as above + `If-Range: "<etag>"` **not** matching | `200`, whole current file, new `ETag` (FR-003) |
| `If-Range: <http-date>` | `200`, whole file |
| range starting at or past the end | `416`, `Content-Range: bytes */size`, empty body (FR-004) |
| several ranges (`bytes=0-1,5-9`) | `200`, whole file (FR-007) |
| unparseable `Range` | `200`, whole file (header ignored) |
| object missing in storage | `404`, as today |

The body is read from storage as it is sent; the server never holds the
whole object to serve a part or a whole (FR-006).

## Built files

`/assets/entry/*`, `/assets/chunks/*`, `/assets/static/*`, served by
`ServeDir` with `.precompressed_br().precompressed_gzip()`.

| Request | Answer |
| --- | --- |
| `Accept-Encoding: br` and `<file>.br` exists | `200`, `Content-Encoding: br`, `Vary: Accept-Encoding`, `Accept-Ranges: bytes`, `ETag` of the `.br` copy |
| `Accept-Encoding: identity` (any `Range` request from a browser) | the original file; ranges as `ServeDir` already serves them (`206`/`416`) |
| no compressed copy (vite dev) | the original file |

The on-the-fly `CompressionLayer` does not touch any of these: a
precompressed answer already has `Content-Encoding`, and a `206` has
`Content-Range`. It also leaves `video/*` and `audio/*` alone.

`If-Range` is not honoured on built files (R3); every name is a content
hash, and the downloader checks the `ETag` of each part itself.
