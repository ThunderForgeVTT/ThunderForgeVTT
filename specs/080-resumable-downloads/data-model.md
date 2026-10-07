# Data Model: Resumable Downloads

No database change. These are the shapes the server answers with and the
downloader holds in memory.

## Version tag

What identifies one exact content of a file.

| Field | Source | Rule |
| --- | --- | --- |
| `etag` | Asset routes: the storage object's S3 `ETag`, quoted. Built files: `ServeDir`'s size+mtime tag. | Strong (no `W/`). Changes when the content changes (FR-002). |
| `lastModified` | `Last-Modified` header | Used as the validator only when there is no strong `etag`. |

The downloader takes the validator from the **first** answer and sends it
as `If-Range` on every part. A part answer is accepted only if it carries
the same `etag` (or, without one, the same `Last-Modified`).

## Part

A contiguous range of one version of one file.

| Field | Type | Rule |
| --- | --- | --- |
| `index` | integer ≥ 0 | Part 0 is the first answer's body. |
| `start` | byte offset | `index × partSize` |
| `end` | byte offset, inclusive | `min(start + partSize, size) − 1` |
| `received` | bytes | Grows only. A retry asks for `start + received ..= end`. |
| `attempts` | integer | Fails the download past `retries` (FR-014, FR-019). |
| `chunks` | `Uint8Array[]` | Held until every earlier part has been handed on. |

**States**: `waiting` → `fetching` → (`retrying` → `fetching`)* →
`complete` → `delivered`. Any part may move the download to `failed` or
`versionChanged`.

## Download

| Field | Type | Rule |
| --- | --- | --- |
| `url` | string | |
| `size` | integer or `null` | From the first answer's `Content-Length`; `null` means one plain request (FR-016). |
| `validator` | Version tag | From the first answer. |
| `mode` | `"plain"` or `"parts"` | `parts` only when size ≥ threshold, `Accept-Ranges: bytes`, a validator, and no `Content-Encoding`. |
| `delivered` | bytes | Bytes handed on in order. |
| `progress` | `{ loaded, total }` | `loaded` = Σ part `received` (never decreases, never double-counts); `total` = `size` or `null` (FR-017). |
| `restarts` | 0 or 1 | A version change before any byte is delivered restarts once. |

**Ends** as one whole stream, or one error:

- `DownloadError { url, status, cause }` — retries exhausted, or a refusal
  (`401`/`403`/`404`), which is not retried.
- `VersionChangedError { url }` — the version changed after bytes were
  delivered downstream (streams only; `downloadBytes` restarts instead).
- `AbortError` — the caller's signal.

## Download settings

One object, defaults in the package (FR-023):

| Setting | Default |
| --- | --- |
| `threshold` | 16 MiB |
| `partSize` | 8 MiB |
| `concurrency` | 4 |
| `retries` | 5 per part |
| `retryDelayMs` | 250, doubling per attempt, capped at 4 s |
| `enabled` | `true` (the flag `feature.download_in_parts` sets it) |

## Feature flag

`feature.download_in_parts` — boolean, default **on**, visible to members.
Off: every download is one plain request (FR-016's fallback forced).
