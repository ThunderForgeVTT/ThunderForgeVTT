/**
 * Spec 037 FR-010 and FR-012, from the review's side.
 *
 * # Why this renders to markup
 *
 * `apps/web` has neither jsdom nor testing-library, and adding either means a
 * lockfile change shared with every other workstream. The review is a pure
 * function of its props, so server-rendering it observes exactly what a person
 * would see. `InteractionAuthor` and `StatusPanel` set this precedent.
 *
 * # The pairing that makes this worth writing
 *
 * `services/__tests__/feedbackPayload.test.ts` asserts the planted secret is
 * absent from the *payload*. This file asserts it is absent from the
 * *review*, of the same buffer, planted the same way. Either assertion alone
 * would pass for a design that redacted somewhere between the two — which is
 * the design `contracts/attachments.md` exists to reject, because it means the
 * person approved bytes other than the ones that were sent.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { FeedbackReview } from "../FeedbackReview";
import { UNKNOWN_DESTINATION } from "@/api/feedback";
import {
  resetLogCaptureForTests,
  snapshot,
  startLogCapture,
} from "@/services/feedbackLogBuffer";
import {
  ALL_INCLUDED,
  type FeedbackSubmissionDraft,
} from "@/services/feedbackPayload";
import type { FeedbackScreenshot } from "@/services/feedbackScreenshot";

const BEARER = "Authorization: Bearer sk-live-9f2ab7c41d0e4b6a8c3f";

const SCREENSHOT: FeedbackScreenshot = {
  pngBase64: "iVBORw0KGgoAAAANSUhEUg==",
  width: 1280,
  height: 720,
  byteSize: 18,
};

function captureWithSecret() {
  const stop = startLogCapture();
  console.error(BEARER);
  console.error("Scene load failed");
  stop();
  return snapshot();
}

function draft(
  overrides: Partial<FeedbackSubmissionDraft> = {},
): FeedbackSubmissionDraft {
  return {
    kind: "ISSUE",
    summary: "Tokens stop moving after a scene change",
    message: "Dragged a token, it snapped back.",
    context: {
      screenPath: "/world/8f14e45f-ceea-467a-9c31-0f5cbb1b9b40/play",
      worldId: "8f14e45f-ceea-467a-9c31-0f5cbb1b9b40",
      clientVersion: "1.0.0",
      browser: "Chrome 141 on Linux",
    },
    logs: captureWithSecret(),
    screenshot: SCREENSHOT,
    inclusions: { ...ALL_INCLUDED },
    ...overrides,
  };
}

function markup(submission: FeedbackSubmissionDraft): string {
  return renderToStaticMarkup(
    <FeedbackReview
      draft={submission}
      destination={UNKNOWN_DESTINATION}
      onToggle={() => undefined}
      onCaptureScreenshot={() => undefined}
      screenshotStatus={null}
      capturing={false}
    />,
  );
}

beforeEach(() => {
  resetLogCaptureForTests();
  vi.spyOn(console, "error").mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  resetLogCaptureForTests();
});

describe("what the person sees is what will be sent", () => {
  it("shows the log bundle and never the secret that was in it", () => {
    const html = markup(draft());

    expect(html).toContain("Scene load failed");
    expect(html).not.toContain("sk-live-9f2ab7c41d0e4b6a8c3f");
    expect(html).toContain("[redacted: bearer token]");
  });

  it("says what was kept, what was dropped and how much was removed (FR-011)", () => {
    const html = markup(draft());

    expect(html).toContain("2 kept");
    expect(html).toContain("nothing dropped");
    expect(html).toContain("already removed before");
  });

  it("shows the message, the screen, the world, the browser and the build", () => {
    const html = markup(draft());

    expect(html).toContain("Dragged a token, it snapped back.");
    expect(html).toContain("/world/8f14e45f-ceea-467a-9c31-0f5cbb1b9b40/play");
    expect(html).toContain("8f14e45f-ceea-467a-9c31-0f5cbb1b9b40");
    expect(html).toContain("Chrome 141 on Linux");
    expect(html).toContain("1.0.0");
  });

  it("shows the screenshot at a size that can be inspected (FR-015)", () => {
    const html = markup(draft());

    expect(html).toContain('data-testid="feedback-screenshot-image"');
    expect(html).toContain(`data:image/png;base64,${SCREENSHOT.pngBase64}`);
    expect(html).toContain("1280×720");
  });
});

describe("a removed part is removed from the payload, not hidden from view", () => {
  it("stops rendering a removed screenshot and says it will not be sent", () => {
    const html = markup(
      draft({ inclusions: { ...ALL_INCLUDED, screenshot: false } }),
    );

    expect(html).not.toContain(`data:image/png;base64,${SCREENSHOT.pngBase64}`);
    expect(html).toContain('data-testid="feedback-removed-screenshot"');
    expect(html).toContain("Removed — this will not be sent.");
  });

  it("marks each removable part with whether it is included", () => {
    const html = markup(
      draft({ inclusions: { ...ALL_INCLUDED, logs: false } }),
    );

    expect(
      /data-testid="feedback-review-logs"[^>]*data-included="false"/.test(html),
    ).toBe(true);
    expect(html).not.toContain("[redacted: bearer token]");
  });
});

describe("the notice sits in this step (FR-014)", () => {
  it("warns rather than reassures when visibility is unknown", () => {
    const html = renderToStaticMarkup(
      <FeedbackReview
        draft={draft()}
        destination={{
          configured: true,
          repository: "thunderforge/vtt",
          isPublic: null,
          visibilityCheckedAt: null,
        }}
        onToggle={() => undefined}
        onCaptureScreenshot={() => undefined}
        screenshotStatus={null}
        capturing={false}
      />,
    );

    // An unconfirmed destination is treated as though it may be public. The
    // opposite default would reassure somebody into sending a screenshot of
    // their table to a repository nobody had checked.
    expect(html).toContain("may be public");
    expect(html).toContain("cannot be recalled");
  });

  it("says an unconfigured instance is keeping it here, and still submits", () => {
    const html = markup(draft());

    expect(html).toContain("No destination is configured");
    expect(html).toContain("cannot be recalled");
  });

  it("says a public destination is public, in those words", () => {
    const html = renderToStaticMarkup(
      <FeedbackReview
        draft={draft()}
        destination={{
          configured: true,
          repository: "thunderforge/vtt",
          isPublic: true,
          visibilityCheckedAt: "2026-09-07T10:00:00Z",
        }}
        onToggle={() => undefined}
        onCaptureScreenshot={() => undefined}
        screenshotStatus={null}
        capturing={false}
      />,
    );

    expect(html).toContain("thunderforge/vtt");
    expect(html).toContain("can be read by anyone");
  });
});
