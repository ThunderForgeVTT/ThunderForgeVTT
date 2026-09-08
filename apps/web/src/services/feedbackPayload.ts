/**
 * What a feedback submission actually is, as bytes (spec 037, FR-010,
 * FR-012, and `contracts/feedback.md`).
 *
 * # The property this module exists to keep
 *
 * `contracts/attachments.md` states the claim FR-012 is judged against: **the
 * bytes the person sees in the review are the bytes that are submitted,
 * because they are the same array.** This module is the single place where a
 * caller could break that, so it is the place that says how.
 *
 * The rule is one sentence: **the review and the payload are built from one
 * snapshot, taken once, and neither is transformed on the way to the other.**
 *
 * - `FeedbackReview` renders the entries of a `LogSnapshot` through
 *   {@link formatLogLine}.
 * - {@link buildFeedbackPayload} serialises **the same** `LogSnapshot` through
 *   **the same** {@link formatLogLine}.
 *
 * So a line that appears in the review appears in the payload, character for
 * character, and there is no second place where a line could be filtered,
 * rewritten, re-redacted or re-read from a buffer that has moved on. Redaction
 * already happened at push time into the ring (`feedbackLogBuffer.ts`); doing
 * it again here would mean the person approved bytes and different bytes were
 * sent, which is precisely the failure FR-012 exists to prevent — **even when
 * the second pass is an improvement**.
 *
 * ## How a future caller would break it
 *
 * By calling `snapshot()` a second time when building the payload instead of
 * passing the snapshot the review was given; by mapping over `entries` here;
 * by adding a "just in case" `redact()` call below. Each looks harmless and
 * each turns the review into a rendering test that proves nothing. The vitest
 * suite beside this file plants a secret in the buffer and asserts its absence
 * in both places for exactly that reason.
 */

import type { LogEntry, LogSnapshot } from "./feedbackLogBuffer";
import type { FeedbackContext } from "./feedbackContext";
import { WITHHELD } from "./feedbackContext";
import type { FeedbackScreenshot } from "./feedbackScreenshot";

export type FeedbackKind = "FEATURE_REQUEST" | "ISSUE" | "GENERAL";

export type FeedbackAttachmentKind = "LOGS" | "SCREENSHOT";

/** One attachment, exactly as `contracts/feedback.md` declares it. */
export interface FeedbackAttachmentInput {
  kind: FeedbackAttachmentKind;
  /** Base64. Logs are UTF-8 text; a screenshot is the PNG the client encoded. */
  content: string;
  entriesKept?: number;
  entriesDropped?: number;
  redactionCount?: number;
}

/** The mutation's input, one field per `contracts/feedback.md`. */
export interface SubmitFeedbackInput {
  kind: FeedbackKind;
  message: string;
  summary?: string;
  screenPath?: string;
  worldId?: string;
  clientVersion: string;
  browser: string;
  attachments: FeedbackAttachmentInput[];
}

/**
 * Which parts the person has left in.
 *
 * FR-010 says **any** part may be removed, so this is one flag per part
 * rather than one flag for "attachments" — a person who is happy to send the
 * logs but not the screen they were on is making a coherent choice, and a
 * coarser model would silently refuse it.
 */
export interface FeedbackInclusions {
  logs: boolean;
  screenshot: boolean;
  screenPath: boolean;
  worldId: boolean;
  browser: boolean;
  clientVersion: boolean;
}

export const ALL_INCLUDED: FeedbackInclusions = {
  logs: true,
  screenshot: true,
  screenPath: true,
  worldId: true,
  browser: true,
  clientVersion: true,
};

/** Everything the review has in front of it, and everything the payload needs. */
export interface FeedbackSubmissionDraft {
  kind: FeedbackKind;
  summary: string;
  message: string;
  context: FeedbackContext;
  /** Taken **once**, when the review opened. Not re-read. */
  logs: LogSnapshot;
  screenshot: FeedbackScreenshot | null;
  inclusions: FeedbackInclusions;
}

/**
 * One log line, as the review shows it and as the attachment contains it.
 *
 * Shared rather than duplicated: two formatters would be two chances for the
 * displayed line and the sent line to differ by a space, and "the same bytes"
 * would then be a claim nobody could check by reading.
 */
export function formatLogLine(entry: LogEntry): string {
  return `${new Date(entry.at).toISOString()} ${entry.level.toUpperCase()} ${entry.text}`;
}

/** The whole bundle as it will be sent: the review's lines, joined. */
export function formatLogBundle(logs: LogSnapshot): string {
  return logs.entries.map(formatLogLine).join("\n");
}

const BASE64_ALPHABET =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/**
 * Base64, written out rather than reached for.
 *
 * `btoa` takes a binary string and throws on any code point above 255, so
 * every caller has to encode to bytes first anyway; and `Buffer` does not
 * exist in a browser. Fifteen lines here is cheaper than either an
 * environment-dependent branch or a dependency, and it makes the encoder
 * testable in the same environment the payload is built in.
 */
export function encodeBase64(bytes: Uint8Array): string {
  let out = "";
  for (let i = 0; i < bytes.length; i += 3) {
    const a = bytes[i];
    const b = i + 1 < bytes.length ? bytes[i + 1] : undefined;
    const c = i + 2 < bytes.length ? bytes[i + 2] : undefined;

    out += BASE64_ALPHABET[a >> 2];
    out += BASE64_ALPHABET[((a & 0x03) << 4) | ((b ?? 0) >> 4)];
    out +=
      b === undefined
        ? "="
        : BASE64_ALPHABET[((b & 0x0f) << 2) | ((c ?? 0) >> 6)];
    out += c === undefined ? "=" : BASE64_ALPHABET[c & 0x3f];
  }
  return out;
}

/** UTF-8, then base64. The attachment's `content` is exactly this. */
export function encodeUtf8Base64(text: string): string {
  return encodeBase64(new TextEncoder().encode(text));
}

/**
 * Turn what the review is showing into what the mutation receives.
 *
 * Pure, synchronous, and total: the same draft always produces the same
 * payload, so "what was reviewed" and "what was sent" can be compared by a
 * test that calls this twice. Nothing here reads a module-scoped buffer,
 * nothing awaits, and nothing redacts.
 */
export function buildFeedbackPayload(
  draft: FeedbackSubmissionDraft,
): SubmitFeedbackInput {
  const { context, inclusions } = draft;
  const attachments: FeedbackAttachmentInput[] = [];

  if (inclusions.logs && draft.logs.entries.length > 0) {
    attachments.push({
      kind: "LOGS",
      content: encodeUtf8Base64(formatLogBundle(draft.logs)),
      entriesKept: draft.logs.entries.length,
      entriesDropped: draft.logs.droppedCount,
      redactionCount: draft.logs.redactionCount,
    });
  }

  if (inclusions.screenshot && draft.screenshot) {
    attachments.push({
      kind: "SCREENSHOT",
      // The very string the review rendered as a data URL. Not re-encoded,
      // not re-drawn: a second encode of the same image is still different
      // bytes, and the person approved these.
      content: draft.screenshot.pngBase64,
    });
  }

  const summary = draft.summary.trim();
  const payload: SubmitFeedbackInput = {
    kind: draft.kind,
    message: draft.message,
    clientVersion: inclusions.clientVersion ? context.clientVersion : WITHHELD,
    browser: inclusions.browser ? context.browser : WITHHELD,
    attachments,
  };

  // GENERAL has no summary field at all (FR-003), so it never carries one
  // even if a kind switch left text behind in the form's state.
  if (draft.kind !== "GENERAL" && summary !== "") {
    payload.summary = summary;
  }

  if (inclusions.screenPath && context.screenPath !== null) {
    payload.screenPath = context.screenPath;
  }

  if (inclusions.worldId && context.worldId !== null) {
    payload.worldId = context.worldId;
  }

  return payload;
}
