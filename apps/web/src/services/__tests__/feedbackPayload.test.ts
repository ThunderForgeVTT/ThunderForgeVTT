/**
 * Spec 037 FR-010 and FR-012, from the payload's side.
 *
 * Two claims are under test here and they are the two that matter:
 *
 * 1. **A secret planted in the log buffer never reaches the payload.** It is
 *    removed at push time by `feedbackRedaction`, so the ring never held it —
 *    which is why this test plants it through `console.error`, the way a real
 *    line arrives, rather than constructing a snapshot by hand. A test that
 *    built its own entries would prove nothing about the path a secret
 *    actually takes.
 * 2. **Removing a part removes it from what would be sent.** SC-003 in one
 *    assertion: flip the inclusion the review's Remove button flips, and the
 *    attachment is gone from the built payload.
 *
 * The companion assertion — that the same secret is absent from the *review* —
 * lives in `components/feedback/__tests__/feedbackReview.test.tsx`, because
 * asserting only this half would pass for a design that redacted on the way
 * out, which is the design FR-012 exists to reject.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  resetLogCaptureForTests,
  snapshot,
  startLogCapture,
  type LogSnapshot,
} from "../feedbackLogBuffer";
import { setSubmitterEmail } from "../feedbackRedaction";
import {
  ALL_INCLUDED,
  buildFeedbackPayload,
  encodeUtf8Base64,
  formatLogBundle,
  type FeedbackSubmissionDraft,
} from "../feedbackPayload";
import { UNKNOWN_BUILD, WITHHELD } from "../feedbackContext";
import type { FeedbackScreenshot } from "../feedbackScreenshot";

const BEARER = "Authorization: Bearer sk-live-9f2ab7c41d0e4b6a8c3f";
const SIGNED_URL =
  "GET https://assets.example/scene.webp?token=AQoDYXdzEJr%2F%2F%2F%2F failed";

const SCREENSHOT: FeedbackScreenshot = {
  pngBase64: "iVBORw0KGgoAAAANSUhEUg==",
  width: 1280,
  height: 720,
  byteSize: 18,
};

function draftWith(logs: LogSnapshot): FeedbackSubmissionDraft {
  return {
    kind: "ISSUE",
    summary: "Tokens stop moving after a scene change",
    message: "Dragged a token, it snapped back.",
    context: {
      screenPath: "/world/8f14e45f-ceea-467a-9c31-0f5cbb1b9b40/play",
      worldId: "8f14e45f-ceea-467a-9c31-0f5cbb1b9b40",
      clientVersion: UNKNOWN_BUILD,
      browser: "Chrome 141 on Linux",
    },
    logs,
    screenshot: SCREENSHOT,
    inclusions: { ...ALL_INCLUDED },
  };
}

/**
 * Base64 back to text, without `Buffer`: this app's tsconfig declares only
 * `vite/client` types, so a Node global would not typecheck even though it
 * would run.
 */
function decode(base64: string): string {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return new TextDecoder().decode(bytes);
}

beforeEach(() => {
  resetLogCaptureForTests();
  setSubmitterEmail(null);
  // The buffer calls the original through, always; silencing it keeps the
  // planted secret out of the test runner's own output.
  vi.spyOn(console, "error").mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  resetLogCaptureForTests();
  setSubmitterEmail(null);
});

describe("a secret in the log buffer never reaches the payload", () => {
  it("drops a bearer token and a signed URL before the ring, so neither can be sent", () => {
    const stop = startLogCapture();
    console.error(BEARER);
    console.error(SIGNED_URL);
    setSubmitterEmail("player@example.test");
    console.error("Upload failed for player@example.test");
    stop();

    const taken = snapshot();
    const payload = buildFeedbackPayload(draftWith(taken));
    const logs = payload.attachments.find((a) => a.kind === "LOGS");
    const sent = decode(logs?.content ?? "");

    expect(sent).not.toContain("sk-live-9f2ab7c41d0e4b6a8c3f");
    expect(sent).not.toContain("AQoDYXdzEJr");
    expect(sent).not.toContain("player@example.test");

    // Removed visibly, not silently: a person is entitled to see that
    // something was taken and what kind of thing it was.
    expect(sent).toContain("[redacted: bearer token]");
    expect(sent).toContain("[redacted: signed url]");
    expect(sent).toContain("[redacted: email]");
    expect(logs?.redactionCount).toBeGreaterThanOrEqual(3);
  });

  it("sends exactly the bytes the snapshot holds — no second pass", () => {
    const stop = startLogCapture();
    console.error(BEARER);
    stop();

    const taken = snapshot();
    const payload = buildFeedbackPayload(draftWith(taken));
    const logs = payload.attachments.find((a) => a.kind === "LOGS");

    // The bundle the review renders, encoded. If anything ever transformed
    // entries between the review and the wire, these two would differ — which
    // is the failure `contracts/attachments.md` is written to make visible.
    expect(logs?.content).toBe(encodeUtf8Base64(formatLogBundle(taken)));
  });

  it("states what was kept and what was dropped (FR-011)", () => {
    const stop = startLogCapture();
    console.error("one");
    console.error("two");
    stop();

    const taken = snapshot();
    const logs = buildFeedbackPayload(draftWith(taken)).attachments.find(
      (a) => a.kind === "LOGS",
    );

    expect(logs?.entriesKept).toBe(taken.entries.length);
    expect(logs?.entriesDropped).toBe(taken.droppedCount);
  });
});

describe("removing a part removes it from what would be sent", () => {
  it("drops the screenshot from the payload when it is removed", () => {
    const stop = startLogCapture();
    console.error("something went wrong");
    stop();

    const draft = draftWith(snapshot());
    expect(
      buildFeedbackPayload(draft).attachments.map((a) => a.kind),
    ).toContain("SCREENSHOT");

    const withoutScreenshot = buildFeedbackPayload({
      ...draft,
      inclusions: { ...draft.inclusions, screenshot: false },
    });

    expect(withoutScreenshot.attachments.map((a) => a.kind)).not.toContain(
      "SCREENSHOT",
    );
    // And nothing else changed with it: removing one part is not a different
    // submission.
    expect(withoutScreenshot.attachments.map((a) => a.kind)).toContain("LOGS");
    expect(withoutScreenshot.message).toBe(draft.message);
  });

  it("drops the logs from the payload when they are removed", () => {
    const stop = startLogCapture();
    console.error(BEARER);
    stop();

    const draft = draftWith(snapshot());
    const payload = buildFeedbackPayload({
      ...draft,
      inclusions: { ...draft.inclusions, logs: false },
    });

    expect(payload.attachments.map((a) => a.kind)).toEqual(["SCREENSHOT"]);
  });

  it("drops the screen and the world when they are removed", () => {
    const draft = draftWith(snapshot());
    const payload = buildFeedbackPayload({
      ...draft,
      inclusions: { ...draft.inclusions, screenPath: false, worldId: false },
    });

    expect(payload.screenPath).toBeUndefined();
    expect(payload.worldId).toBeUndefined();
  });

  it("says a removed browser or build was withheld rather than guessing one", () => {
    const draft = draftWith(snapshot());
    const payload = buildFeedbackPayload({
      ...draft,
      inclusions: { ...draft.inclusions, browser: false, clientVersion: false },
    });

    // The contract declares both non-null, so removal cannot omit them; it
    // says so instead, which is honest in a way an empty string is not.
    expect(payload.browser).toBe(WITHHELD);
    expect(payload.clientVersion).toBe(WITHHELD);
  });
});

describe("each kind carries what it needs and nothing it does not (FR-003)", () => {
  it("never sends a summary for a general message", () => {
    const draft = draftWith(snapshot());
    const payload = buildFeedbackPayload({
      ...draft,
      kind: "GENERAL",
      summary: "left over from switching kinds",
    });

    expect(payload.summary).toBeUndefined();
    expect(payload.kind).toBe("GENERAL");
  });

  it("sends the summary for an issue and a feature request", () => {
    const draft = draftWith(snapshot());
    expect(buildFeedbackPayload(draft).summary).toBe(draft.summary);
    expect(
      buildFeedbackPayload({ ...draft, kind: "FEATURE_REQUEST" }).summary,
    ).toBe(draft.summary);
  });
});

describe("base64", () => {
  it("round-trips text of every remainder length", () => {
    for (const text of ["a", "ab", "abc", "abcd", "→ a multibyte ✓"]) {
      expect(decode(encodeUtf8Base64(text))).toBe(text);
    }
  });
});
