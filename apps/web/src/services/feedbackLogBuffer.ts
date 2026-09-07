/**
 * The browser log ring buffer that an issue report carries with it
 * (spec 037, US2 / FR-007, FR-011, FR-012).
 *
 * # Why intercept `console` rather than route through a façade
 *
 * The errors worth having are the ones nobody wrote a call for: a React
 * render throw, a rejected `fetch` in `graphqlClient.ts`, a wasm panic
 * surfaced by Bevy. Those reach `console.error`, `window.onerror` and
 * `unhandledrejection` and nowhere else — a logging façade would capture only
 * the lines somebody remembered to route through it, which is never the line
 * a bug report wants. (research.md § R5.)
 *
 * # Bounded, so a long session costs what a short one does
 *
 * Three bounds, whichever is reached first: 500 entries, 128 KB total, 2 KB
 * per entry. The capacity is fixed, so a session that logs for eight hours
 * holds the same bytes as one that logs for eight seconds, and one enormous
 * line cannot evict everything that explained it. Eviction is oldest-first
 * and counted, because FR-011 — "when anything is dropped for size the person
 * MUST be told what was kept" — needs a number, not a shrug.
 *
 * # Never written down
 *
 * The ring is a module-scoped array and is never handed to `localStorage`,
 * `sessionStorage`, IndexedDB or OPFS. It dies with the tab. A shared machine
 * inherits nothing from the previous person, no storage-clearing obligation
 * is created, and there is no stored artefact for a later change or a second
 * reader to find. This is not an omission to be tidied up later; it is the
 * property, and `feedbackLogBuffer.test.ts` fails if a write to any of them
 * appears.
 *
 * # Redacted at push, not at submit
 *
 * Every entry passes through `redact()` (FR-012) *before* it enters the ring,
 * so the buffer never holds a secret at any instant. The review renders this
 * buffer and the submission sends this buffer — the same bytes, because they
 * are the same array. Redacting later would mean the person approved
 * something other than what was sent.
 */

import { redact } from "./feedbackRedaction";

export type LogLevel = "error" | "warn" | "info";

export interface LogEntry {
  /** `Date.now()` when the line was captured. */
  at: number;
  level: LogLevel;
  /** Already redacted. There is no unredacted form of this anywhere. */
  text: string;
}

export interface LogSnapshot {
  entries: LogEntry[];
  /** How many entries have been evicted for size since capture started. */
  droppedCount: number;
  /**
   * When the dropped window begins — the timestamp of the *oldest* entry ever
   * evicted, set once. With `entries[0].at` it gives the person both ends of
   * what happened to their logs: everything from here to there is gone, and
   * everything after is what they are looking at.
   */
  droppedOldestAt: number | null;
  /** UTF-8 bytes currently held, against the 128 KB bound. */
  byteSize: number;
  /** How many secrets were removed from what is currently held. */
  redactionCount: number;
}

/** Enough for a session's worth of what went wrong. */
export const MAX_ENTRIES = 500;

/** Fixed, so a four-hour session costs what a four-second one does. */
export const MAX_TOTAL_BYTES = 128 * 1024;

/**
 * One enormous line must not evict everything that explained it. Applied to
 * the captured text; the truncation marker is added on top of it and counted
 * in `byteSize`, so the marker can never be the thing that pushes the
 * interesting half out.
 */
export const MAX_ENTRY_BYTES = 2 * 1024;

interface StoredEntry extends LogEntry {
  /** Kept beside the entry so `redactionCount` survives eviction correctly. */
  redactions: number;
  bytes: number;
}

const encoder = new TextEncoder();

/**
 * The ring. Module-scoped and nothing else: no storage API, no export that
 * hands out the array itself.
 */
let ring: StoredEntry[] = [];
let byteSize = 0;
let redactionCount = 0;
let droppedCount = 0;
let droppedOldestAt: number | null = null;

/** Cut a string to a byte budget without splitting a UTF-8 code point. */
function truncateToBytes(text: string, limit: number): string {
  const encoded = encoder.encode(text);
  if (encoded.length <= limit) {
    return text;
  }

  // Decoding a slice that ends mid-sequence yields U+FFFD; dropping a
  // trailing one is what keeps a truncated emoji from becoming a question
  // mark in the middle of a log line.
  const decoded = new TextDecoder("utf-8").decode(encoded.subarray(0, limit));
  const trimmed = decoded.endsWith("�") ? decoded.slice(0, -1) : decoded;

  return `${trimmed}… [truncated: ${encoded.length - limit} bytes]`;
}

/**
 * Render one console argument. Errors keep their stack, because the stack is
 * the reason the line is worth capturing; everything else is JSON where that
 * works and its own `String()` where it does not (a circular structure, a
 * `BigInt`, a `Proxy` that throws on read).
 */
function describe(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }

  if (value instanceof Error) {
    return value.stack ?? `${value.name}: ${value.message}`;
  }

  try {
    return typeof value === "object" && value !== null
      ? JSON.stringify(value)
      : String(value);
  } catch {
    try {
      return String(value);
    } catch {
      return "[unserialisable]";
    }
  }
}

/** Drop oldest entries until every bound holds again. */
function evict(): void {
  while (ring.length > MAX_ENTRIES || byteSize > MAX_TOTAL_BYTES) {
    const oldest = ring.shift();
    if (!oldest) {
      // Unreachable while the bounds are positive, but a `while` that can
      // spin forever on an empty array is not worth leaving to chance.
      byteSize = 0;
      redactionCount = 0;
      return;
    }

    byteSize -= oldest.bytes;
    redactionCount -= oldest.redactions;
    droppedCount += 1;
    droppedOldestAt ??= oldest.at;
  }
}

/**
 * Capture one line. Redaction happens **here**, before the push, so nothing
 * secret is ever in `ring` — not for a tick, not for a frame.
 */
function push(level: LogLevel, rawText: string): void {
  const redaction = redact(truncateToBytes(rawText, MAX_ENTRY_BYTES));
  const text = redaction.text;
  const bytes = encoder.encode(text).length;

  ring.push({
    at: Date.now(),
    level,
    text,
    redactions: redaction.count,
    bytes,
  });
  byteSize += bytes;
  redactionCount += redaction.count;

  evict();
}

type ConsoleMethod = (...args: unknown[]) => void;

let stopCapture: (() => void) | null = null;

/**
 * Begin capturing.
 *
 * **Idempotent**: a second call while capture is running returns the same
 * stop function and patches nothing again. That matters more than it looks —
 * React's StrictMode double-invokes effects in development, and a second
 * patch over the first would record every line twice and, worse, restore a
 * *patched* console when it was undone.
 *
 * Called once, from `main.tsx`.
 */
export function startLogCapture(): () => void {
  if (stopCapture) {
    return stopCapture;
  }

  const target = console;
  // Held unbound, and restored unbound. A bound copy would restore a
  // *different function* than the one that was there, which makes "stop puts
  // the console back exactly as it found it" untestable and stacks a new
  // wrapper on every start/stop cycle.
  const originals: Record<LogLevel, ConsoleMethod> = {
    error: target.error,
    warn: target.warn,
    info: target.info,
  };

  const levels: LogLevel[] = ["error", "warn", "info"];
  for (const level of levels) {
    target[level] = (...args: unknown[]) => {
      // The original is called through first, always: capture must never
      // change what a developer sees in their own console, and it must not
      // swallow a line if the buffer itself throws.
      originals[level].apply(target, args);
      try {
        push(level, args.map(describe).join(" "));
      } catch {
        // A log line is not worth an exception on the path that was already
        // reporting a problem.
      }
    };
  }

  const onError = (event: ErrorEvent) => {
    const detail = event.error instanceof Error ? describe(event.error) : "";
    const where =
      event.filename !== undefined && event.filename !== ""
        ? ` (${event.filename}:${event.lineno}:${event.colno})`
        : "";
    push(
      "error",
      `Uncaught ${event.message}${where}${detail ? `\n${detail}` : ""}`,
    );
  };

  const onRejection = (event: PromiseRejectionEvent) => {
    push("error", `Unhandled rejection: ${describe(event.reason)}`);
  };

  // `addEventListener`, not `window.onerror = …`: assigning the property
  // would silently replace whatever else had claimed it, and capture has no
  // business taking a slot away from the application it is observing.
  const view: Window | undefined =
    typeof window === "undefined" ? undefined : window;
  view?.addEventListener("error", onError);
  view?.addEventListener("unhandledrejection", onRejection);

  const stop = () => {
    // Identity, not a boolean: a stop function held from an earlier capture
    // must not tear down a later one, and must not restore a console that a
    // second `startLogCapture` has since patched.
    if (stopCapture !== stop) {
      return;
    }
    for (const level of levels) {
      target[level] = originals[level];
    }
    view?.removeEventListener("error", onError);
    view?.removeEventListener("unhandledrejection", onRejection);
    stopCapture = null;
  };

  stopCapture = stop;
  return stop;
}

/** Whether capture is currently installed. */
export function isCapturing(): boolean {
  return stopCapture !== null;
}

/**
 * The current contents: what the review renders **and** what is submitted.
 *
 * Take it once and use that one value for both, so the person cannot approve
 * one buffer and send a later one. The entries are copied out so a caller
 * cannot mutate the ring, but the strings are the same strings — the bytes
 * shown are the bytes sent.
 */
export function snapshot(): LogSnapshot {
  return {
    entries: ring.map((entry) => ({
      at: entry.at,
      level: entry.level,
      text: entry.text,
    })),
    droppedCount,
    droppedOldestAt,
    byteSize,
    redactionCount,
  };
}

/**
 * Forget everything captured so far.
 *
 * Exists for tests, and for the one product case that will want it: a person
 * who has just submitted and does not want the next report to carry the last
 * one's session. It does not stop capture.
 */
export function clearLogBuffer(): void {
  ring = [];
  byteSize = 0;
  redactionCount = 0;
  droppedCount = 0;
  droppedOldestAt = null;
}

/** Test seam: stop capture if it is running, and empty the ring. */
export function resetLogCaptureForTests(): void {
  stopCapture?.();
  clearLogBuffer();
}
