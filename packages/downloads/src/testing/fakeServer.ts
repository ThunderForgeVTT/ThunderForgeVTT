/**
 * A scripted server as a `fetch`, for the downloader's unit tests: no
 * network, no browser. It serves one byte array with ranges, an ETag and
 * `If-Range`, and lets a test delay, cut, refuse or replace answers.
 */

export interface FakeRequest {
  index: number;
  range: string | null;
  ifRange: string | null;
}

export interface Behaviour {
  /** Answer with this status and no body. */
  status?: number;
  /** Fail the body with a network error after this many bytes. */
  cutAfter?: number;
  /** Pause before each chunk. */
  delayMs?: number;
}

export interface FakeServerOptions {
  etag?: string | null;
  lastModified?: string | null;
  acceptRanges?: boolean;
  encoding?: string;
  chunkSize?: number;
  behave?: (request: FakeRequest) => Behaviour | undefined;
}

export interface FakeServer {
  fetch: typeof fetch;
  requests: FakeRequest[];
  /** Bodies currently being sent. */
  readonly open: number;
  /** The most bodies ever sent at once. */
  readonly maxOpen: number;
  replace(bytes: Uint8Array, etag: string): void;
}

const pause = (ms: number) => new Promise((r) => setTimeout(r, ms));

export function fakeServer(
  initial: Uint8Array,
  options: FakeServerOptions = {},
): FakeServer {
  let bytes = initial;
  let etag = options.etag === undefined ? '"v1"' : options.etag;
  const chunkSize = options.chunkSize ?? 3;
  const requests: FakeRequest[] = [];
  let open = 0;
  let maxOpen = 0;

  const fetchFn = async (
    _url: string | URL | Request,
    init: RequestInit = {},
  ) => {
    const headers = new Headers(init.headers);
    const request: FakeRequest = {
      index: requests.length,
      range: headers.get("range"),
      ifRange: headers.get("if-range"),
    };
    requests.push(request);
    const signal = init.signal ?? undefined;
    if (signal?.aborted) throw signal.reason;

    const behaviour = options.behave?.(request) ?? {};
    const out = new Headers();
    if (etag) out.set("etag", etag);
    if (options.lastModified) out.set("last-modified", options.lastModified);
    if (options.acceptRanges !== false) out.set("accept-ranges", "bytes");
    if (options.encoding) out.set("content-encoding", options.encoding);
    if (behaviour.status)
      return new Response(null, { status: behaviour.status, headers: out });

    const served = bytes;
    let start = 0;
    let end = served.length - 1;
    let status = 200;
    const m = request.range && /^bytes=(\d+)-(\d*)$/.exec(request.range);
    const current = etag ?? options.lastModified ?? null;
    const stale = request.ifRange !== null && request.ifRange !== current;
    if (m && !stale && options.acceptRanges !== false) {
      start = Number(m[1]);
      end = m[2]
        ? Math.min(Number(m[2]), served.length - 1)
        : served.length - 1;
      if (start >= served.length) {
        out.set("content-range", `bytes */${served.length}`);
        return new Response(null, { status: 416, headers: out });
      }
      status = 206;
      out.set("content-range", `bytes ${start}-${end}/${served.length}`);
    }
    out.set("content-length", String(end - start + 1));

    let at = start;
    let sent = 0;
    let closed = false;
    const close = () => {
      if (!closed) {
        closed = true;
        open--;
      }
    };
    open++;
    maxOpen = Math.max(maxOpen, open);
    const body = new ReadableStream<Uint8Array>(
      {
        async pull(controller) {
          if (behaviour.delayMs) await pause(behaviour.delayMs);
          if (signal?.aborted) {
            close();
            controller.error(signal.reason);
            return;
          }
          if (at > end) {
            close();
            controller.close();
            return;
          }
          if (behaviour.cutAfter !== undefined && sent >= behaviour.cutAfter) {
            close();
            controller.error(new TypeError("terminated"));
            return;
          }
          let take = Math.min(chunkSize, end - at + 1);
          if (behaviour.cutAfter !== undefined)
            take = Math.min(take, behaviour.cutAfter - sent);
          controller.enqueue(served.slice(at, at + take));
          at += take;
          sent += take;
        },
        cancel() {
          close();
        },
      },
      { highWaterMark: 0 },
    );
    return new Response(body, { status, headers: out });
  };

  return {
    fetch: fetchFn as typeof fetch,
    requests,
    get open() {
      return open;
    },
    get maxOpen() {
      return maxOpen;
    },
    replace(next, nextEtag) {
      bytes = next;
      etag = nextEtag;
    },
  };
}

/** `n` bytes whose value is their offset mod 251. */
export function patterned(n: number, salt = 0): Uint8Array {
  const out = new Uint8Array(n);
  for (let i = 0; i < n; i++) out[i] = (i + salt) % 251;
  return out;
}
