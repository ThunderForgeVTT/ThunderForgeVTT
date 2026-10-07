# Contract: `@thunderforge/downloads`

No React, engine or app import. Works in a browser, a worker and Node 22+.

```ts
export interface DownloadSettings {
  threshold: number;     // bytes; default 16 MiB
  partSize: number;      // bytes; default 8 MiB
  concurrency: number;   // default 4
  retries: number;       // per part; default 5
  retryDelayMs: number;  // first pause; doubles, capped at 4000; default 250
  enabled: boolean;      // false = always one plain request; default true
}
export const DEFAULT_DOWNLOAD_SETTINGS: Readonly<DownloadSettings>;

export interface DownloadProgress { loaded: number; total: number | null; }

export interface DownloadOptions {
  settings?: Partial<DownloadSettings>;
  signal?: AbortSignal;
  onProgress?: (p: DownloadProgress) => void;
  init?: RequestInit;            // headers, credentials, cache for every request
  fetch?: typeof fetch;          // injected for tests
}

export interface Download {
  /** Bytes in file order. Errors with DownloadError / VersionChangedError / AbortError. */
  body: ReadableStream<Uint8Array>;
  /** Headers of the first answer (content-type, etag, content-length). */
  headers: Headers;
  total: number | null;
  mode: "plain" | "parts";
  /** A Response over `body` with the first answer's status 200 and headers,
   *  for consumers that take a Response (WebAssembly.compileStreaming). */
  toResponse(): Response;
}

export function download(url: string, options?: DownloadOptions): Promise<Download>;
export function downloadBytes(url: string, options?: DownloadOptions): Promise<Uint8Array>;

export class DownloadError extends Error { url: string; status: number | null; }
export class VersionChangedError extends Error { url: string; }
```

## Rules

- `download()` resolves once the first answer's headers are in. A first
  answer that is not `2xx` rejects with `DownloadError` (status kept, body
  not read), so a refusal is never retried as a part.
- `mode` is `"parts"` only when the first answer is `200`, `size ≥
  threshold`, `Accept-Ranges: bytes`, has a validator, no
  `Content-Encoding` (other than `identity`), and `enabled`.
- `toResponse()` sets `Content-Length` to the full size and drops
  `Content-Encoding` (the body is already decoded).
- `downloadBytes()` restarts once on a version change, then fails.
- Progress `loaded` is non-decreasing; it is called at most once per
  chunk received; `total` is the size or `null`.

## Page bridge (engine)

```ts
globalThis.__thunderforgeDownloadBytes?: (url: string) => Promise<Uint8Array>;
```

Installed by the engine loader before the engine starts. The engine's
world cache calls it when present and falls back to its own fetch.

## Development override

`globalThis.__thunderforgeDownloadSettings?: Partial<DownloadSettings>`,
read by the app's wiring only when `import.meta.env.DEV`. The e2e slice
sets a small `threshold`/`partSize`.
