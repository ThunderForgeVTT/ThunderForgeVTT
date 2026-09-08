/**
 * Everything that will be sent, before it is sent (spec 037, FR-010, FR-011,
 * FR-014, FR-015, and SC-003).
 *
 * # This component renders the payload; it does not prepare one
 *
 * It is handed the same `FeedbackSubmissionDraft` that
 * `buildFeedbackPayload` is handed, and it renders the log lines through the
 * same `formatLogLine` that serialises them. There is deliberately no
 * filtering, no truncation for display and no "prettier" rendering of an
 * entry: the moment this file transforms a line, the review stops being
 * evidence of what is sent and becomes a picture of something adjacent to it.
 * `contracts/attachments.md` § 4 is this component's specification.
 *
 * Removing a part removes it from the **payload**, not from the display of the
 * payload — the `inclusions` flags this renders are the same flags the builder
 * reads.
 */

import { useMemo, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import type { FeedbackDestinationNotice } from "@/api/feedback";
import {
  formatLogLine,
  type FeedbackInclusions,
  type FeedbackSubmissionDraft,
} from "@/services/feedbackPayload";
import { definitionFor } from "./feedbackKinds";

interface FeedbackReviewProps {
  draft: FeedbackSubmissionDraft;
  destination: FeedbackDestinationNotice;
  onToggle: (part: keyof FeedbackInclusions, next: boolean) => void;
  onCaptureScreenshot: () => void;
  /** A plain line when an offer produced nothing. Never an error state. */
  screenshotStatus: string | null;
  capturing: boolean;
}

function formatBytes(bytes: number): string {
  return bytes < 1024 ? `${bytes} bytes` : `${Math.round(bytes / 1024)} KB`;
}

function Removable({
  label,
  testId,
  included,
  onToggle,
  children,
}: {
  label: string;
  testId: string;
  included: boolean;
  onToggle: (next: boolean) => void;
  children: ReactNode;
}) {
  return (
    <div
      data-testid={`feedback-review-${testId}`}
      data-included={included}
      className="grid gap-1 rounded-lg border border-border p-3"
    >
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs font-medium text-muted-foreground uppercase">
          {label}
        </span>
        <Button
          type="button"
          size="xs"
          variant={included ? "ghost" : "outline"}
          data-testid={`feedback-toggle-${testId}`}
          aria-pressed={!included}
          onClick={() => onToggle(!included)}
        >
          {included ? "Remove" : "Put back"}
        </Button>
      </div>
      {included ? (
        children
      ) : (
        <p
          className="text-sm text-muted-foreground italic"
          data-testid={`feedback-removed-${testId}`}
        >
          Removed — this will not be sent.
        </p>
      )}
    </div>
  );
}

function DestinationNotice({
  destination,
}: {
  destination: FeedbackDestinationNotice;
}) {
  const visibility = !destination.configured
    ? "Nothing is configured to receive this yet, so it will be kept here until something is."
    : destination.isPublic === true
      ? "That repository is public: anything you send here can be read by anyone."
      : destination.isPublic === false
        ? "That repository is private, as last observed."
        : "Its visibility has not been confirmed — treat it as though it may be public.";

  return (
    <div
      data-testid="feedback-destination-notice"
      className="grid gap-1 rounded-lg border border-border bg-muted/40 p-3 text-sm"
    >
      <p className="font-medium">Where this goes</p>
      <p data-testid="feedback-destination-repository">
        {destination.configured && destination.repository
          ? `An issue in ${destination.repository}.`
          : "No destination is configured on this instance."}
      </p>
      <p data-testid="feedback-destination-visibility">{visibility}</p>
      {destination.visibilityCheckedAt ? (
        <p className="text-xs text-muted-foreground">
          Visibility last checked{" "}
          {new Date(destination.visibilityCheckedAt).toLocaleString()}.
        </p>
      ) : null}
      <p data-testid="feedback-destination-permanence">
        Once it is sent it is out of this instance&rsquo;s hands: it cannot be
        recalled, edited or deleted from there by us.
      </p>
    </div>
  );
}

export function FeedbackReview({
  draft,
  destination,
  onToggle,
  onCaptureScreenshot,
  screenshotStatus,
  capturing,
}: FeedbackReviewProps) {
  const definition = definitionFor(draft.kind);
  const { logs, context, inclusions } = draft;

  // The exact lines the attachment will contain. Memoised on the snapshot
  // object itself, which never changes once the review has opened — that is
  // what makes "the same array" a thing this component can be trusted about.
  const lines = useMemo(() => logs.entries.map(formatLogLine), [logs]);

  return (
    <div className="grid gap-3" data-testid="feedback-review">
      <p className="text-sm text-muted-foreground">
        This is everything that will be sent. Remove any part of it and send the
        rest.
      </p>

      {/*
        The message is not removable, and that is not an exception to FR-010:
        a submission with its message removed is not a submission. Everything
        the *app* collected can be dropped; the words are the thing being
        sent.
      */}
      <div
        data-testid="feedback-review-message"
        className="grid gap-1 rounded-lg border border-border p-3"
      >
        <span className="text-xs font-medium text-muted-foreground uppercase">
          {definition.label} — your words
        </span>
        {definition.hasSummary && draft.summary.trim() !== "" ? (
          <p className="font-medium" data-testid="feedback-review-summary">
            {draft.summary}
          </p>
        ) : null}
        <p className="whitespace-pre-wrap" data-testid="feedback-review-body">
          {draft.message}
        </p>
        <p className="text-xs text-muted-foreground">
          Your own words are never edited or redacted — what you typed is what
          is sent, which is why you are seeing it here.
        </p>
      </div>

      <Removable
        label="Screen"
        testId="screen"
        included={inclusions.screenPath}
        onToggle={(next) => onToggle("screenPath", next)}
      >
        <p data-testid="feedback-value-screen" className="font-mono text-sm">
          {context.screenPath ?? "no screen recorded"}
        </p>
      </Removable>

      {context.worldId ? (
        <Removable
          label="World"
          testId="world"
          included={inclusions.worldId}
          onToggle={(next) => onToggle("worldId", next)}
        >
          <p data-testid="feedback-value-world" className="font-mono text-sm">
            {context.worldId}
          </p>
          <p className="text-xs text-muted-foreground">
            The game system is looked up from this world when it arrives; it is
            not something this form sends.
          </p>
        </Removable>
      ) : null}

      <Removable
        label="Browser"
        testId="browser"
        included={inclusions.browser}
        onToggle={(next) => onToggle("browser", next)}
      >
        <p data-testid="feedback-value-browser">{context.browser}</p>
      </Removable>

      <Removable
        label="Build"
        testId="version"
        included={inclusions.clientVersion}
        onToggle={(next) => onToggle("clientVersion", next)}
      >
        <p data-testid="feedback-value-version">{context.clientVersion}</p>
      </Removable>

      <Removable
        label={`Browser logs (${lines.length} lines, ${formatBytes(logs.byteSize)})`}
        testId="logs"
        included={inclusions.logs}
        onToggle={(next) => onToggle("logs", next)}
      >
        <p
          className="text-xs text-muted-foreground"
          data-testid="feedback-review-log-counts"
        >
          {lines.length} kept
          {logs.droppedCount > 0
            ? `, ${logs.droppedCount} older lines dropped for size${
                logs.droppedOldestAt
                  ? ` (from ${new Date(logs.droppedOldestAt).toLocaleTimeString()})`
                  : ""
              }`
            : ", nothing dropped"}
          {logs.redactionCount > 0
            ? `, ${logs.redactionCount} secret${logs.redactionCount === 1 ? "" : "s"} already removed before ${logs.redactionCount === 1 ? "it was" : "they were"} recorded`
            : ", no secrets found to remove"}
          .
        </p>
        {lines.length === 0 ? (
          <p
            className="text-sm text-muted-foreground"
            data-testid="feedback-review-log-empty"
          >
            Nothing was logged this session.
          </p>
        ) : (
          <pre
            data-testid="feedback-log-bundle"
            className="max-h-64 overflow-auto rounded-md bg-muted p-2 font-mono text-xs whitespace-pre-wrap"
          >
            {lines.join("\n")}
          </pre>
        )}
      </Removable>

      <Removable
        label="Screenshot"
        testId="screenshot"
        included={inclusions.screenshot}
        onToggle={(next) => onToggle("screenshot", next)}
      >
        {draft.screenshot ? (
          <>
            <img
              data-testid="feedback-screenshot-image"
              alt="The screenshot you captured, as it will be sent"
              src={`data:image/png;base64,${draft.screenshot.pngBase64}`}
              className="max-h-96 w-full rounded-md border border-border object-contain"
            />
            <p className="text-xs text-muted-foreground">
              {draft.screenshot.width}×{draft.screenshot.height},{" "}
              {formatBytes(draft.screenshot.byteSize)}. Look at it before you
              send it — it will show whatever you shared.
            </p>
          </>
        ) : (
          <div className="grid gap-2">
            <p className="text-sm text-muted-foreground">
              No screenshot. Your browser will ask you what to share, and you
              can send this without one.
            </p>
            <div>
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled={capturing}
                data-testid="feedback-capture-screenshot"
                onClick={onCaptureScreenshot}
              >
                {capturing ? "Waiting for you…" : "Take a screenshot"}
              </Button>
            </div>
          </div>
        )}
        {screenshotStatus ? (
          <p
            className="text-xs text-muted-foreground"
            data-testid="feedback-screenshot-status"
          >
            {screenshotStatus}
          </p>
        ) : null}
      </Removable>

      <DestinationNotice destination={destination} />
    </div>
  );
}
