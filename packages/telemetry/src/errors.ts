/**
 * An error, as the one free-text record that leaves (FR-016).
 *
 * The message and the stack are the only free text a browser sends, so both
 * go through the app's `Redactor` and are cut: 512 characters for the
 * message, 4096 for the stack. A stack frame becomes `path:line`, with no
 * origin, no query string and no column.
 */

import type { AttrValue } from "./allowList.ts";

export type Redactor = (text: string) => string;

export type ErrorSource =
  | "boundary"
  | "onerror"
  | "unhandledrejection"
  | "engine";

export const MESSAGE_CAP = 512;
export const STACK_CAP = 4096;

function cut(text: string, max: number): string {
  return text.length > max ? text.slice(0, max) : text;
}

/** One frame's location as `path:line`, or null when it has none. */
function frameLocation(line: string): string | null {
  const m =
    /((?:[a-z][a-z0-9+.-]*:\/\/[^\s/)]*)?[^\s()@]*?):(\d+)(?::\d+)?\)?\s*$/i.exec(
      line.trim(),
    );
  if (!m) return null;
  let path = m[1];
  path = path.replace(/^[a-z][a-z0-9+.-]*:\/\/[^/]*/i, "");
  path = path.replace(/[?#].*$/, "");
  if (!path) return null;
  return `${path}:${m[2]}`;
}

/** A stack reduced to `path:line` frames, one per line. */
export function reduceStack(stack: string | undefined): string {
  if (!stack) return "";
  const frames: string[] = [];
  for (const line of stack.split("\n")) {
    const loc = frameLocation(line);
    if (loc) frames.push(loc);
  }
  return frames.join("\n");
}

function describe(error: unknown): {
  type: string;
  message: string;
  stack?: string;
} {
  if (error instanceof Error) {
    return {
      type: error.name || "Error",
      message: error.message,
      stack: error.stack,
    };
  }
  if (typeof error === "string") return { type: "string", message: error };
  if (error && typeof error === "object") {
    const e = error as { name?: unknown; message?: unknown; stack?: unknown };
    return {
      type: typeof e.name === "string" ? e.name : "object",
      message: typeof e.message === "string" ? e.message : "",
      stack: typeof e.stack === "string" ? e.stack : undefined,
    };
  }
  return { type: typeof error, message: String(error) };
}

/** The attributes of an `error` record, redacted and cut. */
export function errorAttributes(
  source: ErrorSource,
  error: unknown,
  redact: Redactor,
): Record<string, AttrValue> {
  const d = describe(error);
  const safe = (text: string) => {
    try {
      return redact(text);
    } catch {
      return "";
    }
  };
  return {
    "error.source": source,
    "error.type": cut(d.type, 128),
    "error.message": cut(safe(d.message), MESSAGE_CAP),
    "error.stack": cut(safe(reduceStack(d.stack)), STACK_CAP),
  };
}

/** The key two errors fold under: their type and redacted message. */
export function foldKey(attrs: Record<string, AttrValue>): string {
  return `${attrs["error.source"]}|${attrs["error.type"]}|${attrs["error.message"]}`;
}

const EMAIL = /[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+/g;
const UUID = /\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b/gi;

/**
 * The app's redactor, then any email and any UUID. The feedback rule set
 * matches only the submitter's own email, and the disclosure promises that
 * no email and no id leaves, so the core adds both whatever port it is given.
 * The server's `telemetry::redact` does the same.
 */
export function withPersonalDetailsRemoved(redact: Redactor): Redactor {
  return (text) =>
    redact(text).replace(EMAIL, "[redacted: email]").replace(UUID, "[redacted: id]");
}
