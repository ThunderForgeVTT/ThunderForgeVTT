/**
 * Spec 080: one large file as parts, handed on in order.
 *
 * Part 0 is the body of the plain first answer, read until it has given a
 * part's worth of bytes; parts 1..n are ranged requests. At most
 * `concurrency` parts are open at once, counted from the part the reader is
 * waiting on, so what is held is bounded by `concurrency × partSize` and
 * never by the file (FR-013).
 */
import { DownloadError, VersionChangedError } from "./errors.ts";
import { retryDelay, type DownloadSettings } from "./settings.ts";

/** What identifies the version every part must come from (FR-015). */
export interface Validator {
  etag: string | null;
  lastModified: string | null;
}

export interface PartsInput {
  url: string;
  size: number;
  validator: Validator;
  settings: DownloadSettings;
  fetch: typeof fetch;
  init: RequestInit;
  /** The first answer's body; part 0. */
  first: ReadableStreamDefaultReader<Uint8Array>;
  /** Aborted when the download is cancelled or fails. */
  signal: AbortSignal;
  abort: (reason: unknown) => void;
  /** Called with each new byte count received from the network. */
  onBytes: (n: number) => void;
}

interface Part {
  start: number;
  /** Inclusive. */
  end: number;
  received: number;
  chunks: Uint8Array[];
  done: boolean;
}

/** A failure worth another try of the same part. */
class Retryable extends Error {
  readonly status: number | null;
  constructor(message: string, status: number | null) {
    super(message);
    this.status = status;
  }
}

const REFUSED = new Set([401, 403, 404, 410]);

export function partsStream(input: PartsInput): ReadableStream<Uint8Array> {
  const { size, settings } = input;
  const count = Math.ceil(size / settings.partSize);
  const parts: (Part | undefined)[] = [];
  let next = 0;
  let failure: unknown = null;
  let wake: (() => void) | null = null;

  const notify = () => {
    const w = wake;
    wake = null;
    w?.();
  };

  const fail = (error: unknown) => {
    if (failure === null) {
      failure = error;
      input.abort(error);
    }
    notify();
  };

  const startAhead = () => {
    const last = Math.min(count, next + settings.concurrency);
    for (let i = next; i < last; i++) {
      if (parts[i]) continue;
      const start = i * settings.partSize;
      const part: Part = {
        start,
        end: Math.min(start + settings.partSize, size) - 1,
        received: 0,
        chunks: [],
        done: false,
      };
      parts[i] = part;
      runPart(input, part, i, notify).catch(fail);
    }
  };

  return new ReadableStream<Uint8Array>(
    {
      async pull(controller) {
        for (;;) {
          if (failure !== null) throw failure;
          if (next >= count) {
            controller.close();
            return;
          }
          startAhead();
          const part = parts[next]!;
          const chunk = part.chunks.shift();
          if (chunk) {
            controller.enqueue(chunk);
            return;
          }
          if (part.done) {
            parts[next] = undefined;
            next++;
            continue;
          }
          // Checked and armed in the same tick, so no notification is lost.
          await new Promise<void>((resolve) => {
            wake = resolve;
          });
        }
      },
      cancel(reason) {
        input.abort(reason);
      },
    },
    { highWaterMark: 0 },
  );
}

async function runPart(
  input: PartsInput,
  part: Part,
  index: number,
  notify: () => void,
): Promise<void> {
  const { url, settings, signal } = input;
  const length = part.end - part.start + 1;
  let attempt = 0;
  let lastStatus: number | null = null;

  for (;;) {
    try {
      if (index === 0 && attempt === 0) {
        await readInto(input, part, input.first, notify);
      } else {
        const from = part.start + part.received;
        const headers = new Headers(input.init.headers);
        headers.set("Range", `bytes=${from}-${part.end}`);
        const tag = input.validator.etag ?? input.validator.lastModified;
        if (tag) headers.set("If-Range", tag);
        const response = await input.fetch(url, {
          ...input.init,
          headers,
          signal,
        });
        lastStatus = response.status;
        checkPart(input, response, from, part.end);
        await readInto(input, part, response.body!.getReader(), notify);
      }
      if (part.received < length) {
        throw new Retryable(
          `part ended at ${part.received} of ${length} bytes`,
          lastStatus,
        );
      }
      part.done = true;
      notify();
      return;
    } catch (error) {
      if (signal.aborted) throw signal.reason;
      if (
        error instanceof VersionChangedError ||
        error instanceof DownloadError
      )
        throw error;
      attempt++;
      if (attempt > settings.retries) {
        throw new DownloadError(url, lastStatus, undefined, error);
      }
      await sleep(retryDelay(settings, attempt), signal);
    }
  }
}

/** Accepts a part answer, or says why not. */
function checkPart(
  input: PartsInput,
  response: Response,
  from: number,
  end: number,
): void {
  const { url, size, validator } = input;
  const discard = () => response.body?.cancel().catch(() => {});

  if (REFUSED.has(response.status)) {
    discard();
    throw new DownloadError(url, response.status);
  }
  // A whole answer means the server dropped the range: the version changed
  // (If-Range), or it stopped offering parts. A 416 means the file shrank.
  if (response.status === 200 || response.status === 416) {
    discard();
    throw new VersionChangedError(url);
  }
  if (response.status !== 206) {
    discard();
    throw new Retryable(`part answered ${response.status}`, response.status);
  }
  const range = /^bytes (\d+)-(\d+)\/(\d+)$/.exec(
    (response.headers.get("content-range") ?? "").trim(),
  );
  const sameVersion = validator.etag
    ? response.headers.get("etag") === validator.etag
    : response.headers.get("last-modified") === validator.lastModified;
  if (!range || Number(range[3]) !== size || !sameVersion) {
    discard();
    throw new VersionChangedError(url);
  }
  if (Number(range[1]) !== from || Number(range[2]) !== end) {
    discard();
    throw new Retryable(
      `asked for ${from}-${end}, got ${range[1]}-${range[2]}`,
      206,
    );
  }
}

async function readInto(
  input: PartsInput,
  part: Part,
  reader: ReadableStreamDefaultReader<Uint8Array>,
  notify: () => void,
): Promise<void> {
  const length = part.end - part.start + 1;
  try {
    while (part.received < length) {
      const { done, value } = await reader.read();
      if (done) return;
      const chunk =
        value.byteLength > length - part.received
          ? value.subarray(0, length - part.received)
          : value;
      if (chunk.byteLength === 0) continue;
      part.chunks.push(chunk);
      part.received += chunk.byteLength;
      input.onBytes(chunk.byteLength);
      notify();
    }
  } finally {
    // Part 0 is a whole-file answer: stop it once it has given its part.
    reader.cancel().catch(() => {});
  }
}

function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) return reject(signal.reason);
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, ms);
    const onAbort = () => {
      clearTimeout(timer);
      reject(signal.reason);
    };
    signal.addEventListener("abort", onAbort, { once: true });
  });
}
