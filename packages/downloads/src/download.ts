/**
 * Spec 080: the downloader every large download in the web app goes
 * through (FR-011).
 *
 * The first request is the plain one the caller would have made anyway, so
 * a small file, a compressed answer or a server without ranges is exactly
 * one request, cached by the browser as before (FR-016). Only a large,
 * uncompressed answer that offers ranges and names its version continues in
 * parts (research R5, R6).
 */
import { DownloadError, VersionChangedError } from "./errors.ts";
import { partsStream } from "./parts.ts";
import { resolveSettings, type DownloadSettings } from "./settings.ts";

export interface DownloadProgress {
  /** Bytes received so far. Never decreases. */
  loaded: number;
  /** The file's size, or `null` when it is not known (FR-017). */
  total: number | null;
}

export interface DownloadOptions {
  settings?: Partial<DownloadSettings>;
  signal?: AbortSignal;
  onProgress?: (progress: DownloadProgress) => void;
  /** Headers, credentials and cache mode for every request made. */
  init?: RequestInit;
  /** Injected in tests. */
  fetch?: typeof fetch;
}

export interface Download {
  /** The file's bytes in order. Errors rather than ending short (FR-019). */
  body: ReadableStream<Uint8Array>;
  /** The first answer's headers. */
  headers: Headers;
  total: number | null;
  mode: "plain" | "parts";
  /** A `200` over `body`, for consumers that take a `Response`. */
  toResponse(): Response;
}

export async function download(
  url: string,
  options: DownloadOptions = {},
): Promise<Download> {
  const settings = resolveSettings(options.settings);
  const doFetch = options.fetch ?? globalThis.fetch.bind(globalThis);
  const init = options.init ?? {};

  // One controller for every request of this download: the caller's signal,
  // a cancelled body and a failed part all end up here (FR-018).
  const controller = new AbortController();
  const outer = options.signal;
  if (outer) {
    if (outer.aborted) throw outer.reason;
    outer.addEventListener("abort", () => controller.abort(outer.reason), {
      once: true,
    });
  }
  const abort = (reason: unknown) => {
    if (!controller.signal.aborted) controller.abort(reason);
  };

  let first: Response;
  try {
    first = await doFetch(url, { ...init, signal: controller.signal });
  } catch (error) {
    if (controller.signal.aborted) throw controller.signal.reason;
    throw new DownloadError(url, null, undefined, error);
  }
  if (!first.ok) {
    first.body?.cancel().catch(() => {});
    throw new DownloadError(url, first.status);
  }

  const headers = first.headers;
  const encoding = (headers.get("content-encoding") ?? "").trim().toLowerCase();
  const encoded = encoding !== "" && encoding !== "identity";
  const lengthHeader = headers.get("content-length")?.trim() ?? "";
  const length = /^\d+$/.test(lengthHeader) ? Number(lengthHeader) : null;
  // A compressed answer's length counts compressed bytes, but the body
  // yields decoded ones, so its total is unknown.
  const total = encoded ? null : length;
  const etag = headers.get("etag");
  const validator = {
    etag: etag && !etag.startsWith("W/") ? etag : null,
    lastModified: headers.get("last-modified"),
  };

  let loaded = 0;
  const onBytes = (n: number) => {
    loaded += n;
    options.onProgress?.({ loaded, total });
  };

  const inParts =
    settings.enabled &&
    first.status === 200 &&
    first.body !== null &&
    !encoded &&
    length !== null &&
    length >= settings.threshold &&
    /\bbytes\b/i.test(headers.get("accept-ranges") ?? "") &&
    (validator.etag !== null || validator.lastModified !== null);

  const body = inParts
    ? partsStream({
        url,
        size: length!,
        validator,
        settings,
        fetch: doFetch,
        init,
        first: first.body!.getReader(),
        signal: controller.signal,
        abort,
        onBytes,
      })
    : plainStream(url, first, controller.signal, abort, onBytes);

  return {
    body,
    headers,
    total,
    mode: inParts ? "parts" : "plain",
    toResponse() {
      const out = new Headers(headers);
      out.delete("content-encoding");
      out.delete("content-range");
      if (total === null) out.delete("content-length");
      else out.set("content-length", String(total));
      return new Response(body, { status: 200, headers: out });
    },
  };
}

/** The first answer as it is, counted (FR-016). */
function plainStream(
  url: string,
  first: Response,
  signal: AbortSignal,
  abort: (reason: unknown) => void,
  onBytes: (n: number) => void,
): ReadableStream<Uint8Array> {
  if (!first.body) return new ReadableStream({ start: (c) => c.close() });
  const reader = first.body.getReader();
  return new ReadableStream<Uint8Array>(
    {
      async pull(controller) {
        let result: ReadableStreamReadResult<Uint8Array>;
        try {
          result = await reader.read();
        } catch (error) {
          if (signal.aborted) throw signal.reason;
          throw new DownloadError(url, first.status, undefined, error);
        }
        if (result.done) {
          controller.close();
          return;
        }
        onBytes(result.value.byteLength);
        controller.enqueue(result.value);
      },
      cancel(reason) {
        reader.cancel(reason).catch(() => {});
        abort(reason);
      },
    },
    { highWaterMark: 0 },
  );
}

/**
 * The whole file in memory. A version change starts the download again
 * once, on the new version (FR-015); progress still never goes backwards.
 */
export async function downloadBytes(
  url: string,
  options: DownloadOptions = {},
): Promise<Uint8Array> {
  let shown = 0;
  const onProgress = options.onProgress
    ? (p: DownloadProgress) => {
        shown = Math.max(shown, p.loaded);
        options.onProgress!({ loaded: shown, total: p.total });
      }
    : undefined;

  for (let attempt = 0; ; attempt++) {
    try {
      const got = await download(url, { ...options, onProgress });
      return await collect(got.body, got.total);
    } catch (error) {
      if (error instanceof VersionChangedError && attempt === 0) continue;
      throw error;
    }
  }
}

async function collect(
  body: ReadableStream<Uint8Array>,
  total: number | null,
): Promise<Uint8Array> {
  const reader = body.getReader();
  if (total !== null) {
    const out = new Uint8Array(total);
    let at = 0;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      if (at + value.byteLength > total)
        throw new DownloadError("", null, "more bytes than announced");
      out.set(value, at);
      at += value.byteLength;
    }
    if (at !== total)
      throw new DownloadError("", null, `ended at ${at} of ${total} bytes`);
    return out;
  }
  const chunks: Uint8Array[] = [];
  let size = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    size += value.byteLength;
  }
  const out = new Uint8Array(size);
  let at = 0;
  for (const c of chunks) {
    out.set(c, at);
    at += c.byteLength;
  }
  return out;
}
