/**
 * The feedback form: choose a kind, say the thing, review what leaves
 * (spec 037, US1 and US2).
 *
 * # Two steps, and the second one is the feature
 *
 * Compose, then review. The review is not a confirmation dialog — it is where
 * FR-010 lives, and the reason the form has a step at all. `FeedbackReview`
 * renders it; this component owns the state it renders and hands the *same*
 * state to `buildFeedbackPayload`.
 *
 * # The snapshot is taken once
 *
 * `snapshot()` is called at the moment the review opens and the result is held
 * in state until the dialog closes. It is **not** re-read at submit time.
 * That is the whole of `contracts/attachments.md`'s claim: the review renders
 * this object and the payload serialises this object, so a line cannot appear
 * in one and not the other, and the buffer moving on between approving and
 * sending cannot change what was approved.
 *
 * # The draft outlives a dismissal, and a refusal
 *
 * FR-005 and FR-006's second clause are the same mechanism: the draft is
 * written to `sessionStorage` as it is typed and cleared **only** on a
 * successful submission. Closing the dialog, reloading the form, or being
 * refused by the rate limiter all leave the words where the person put them.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  UNKNOWN_DESTINATION,
  containsSecret,
  getFeedbackDestinationNotice,
  isRateLimited,
  submitFeedback,
  type FeedbackDestinationNotice,
} from "@/api/feedback";
import { collectFeedbackContext } from "@/services/feedbackContext";
import { snapshot, type LogSnapshot } from "@/services/feedbackLogBuffer";
import {
  ALL_INCLUDED,
  buildFeedbackPayload,
  type FeedbackInclusions,
  type FeedbackKind,
  type FeedbackSubmissionDraft,
} from "@/services/feedbackPayload";
import {
  captureScreenshot,
  type FeedbackScreenshot,
} from "@/services/feedbackScreenshot";
import {
  clearDraft,
  loadDraft,
  saveDraft,
  type FeedbackDraft,
} from "@/services/feedbackDraft";
import { FEEDBACK_KINDS, definitionFor } from "./feedbackKinds";
import { FeedbackReview } from "./FeedbackReview";

interface FeedbackDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The route the person is on, from the router above every screen. */
  pathname: string;
}

type Step = "compose" | "review" | "sent";

export function FeedbackDialog({
  open,
  onOpenChange,
  pathname,
}: FeedbackDialogProps) {
  const [draft, setDraft] = useState<FeedbackDraft>(() => loadDraft());
  const [step, setStep] = useState<Step>("compose");
  const [logs, setLogs] = useState<LogSnapshot | null>(null);
  const [screenshot, setScreenshot] = useState<FeedbackScreenshot | null>(null);
  const [screenshotStatus, setScreenshotStatus] = useState<string | null>(null);
  const [capturing, setCapturing] = useState(false);
  const [inclusions, setInclusions] =
    useState<FeedbackInclusions>(ALL_INCLUDED);
  const [destination, setDestination] =
    useState<FeedbackDestinationNotice>(UNKNOWN_DESTINATION);
  const [submitting, setSubmitting] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  const definition = definitionFor(draft.kind);

  /*
    The notice is read once, when the form opens (FR-014 wants it on screen
    while the person decides, not after). This component is mounted *by* the
    launcher only while the form is open, so mounting is opening — which is
    also why the draft above is read in a `useState` initialiser rather than
    in an effect: there is no stale copy to re-read.
  */
  useEffect(() => {
    let live = true;
    void getFeedbackDestinationNotice().then((notice) => {
      if (live) {
        setDestination(notice);
      }
    });
    return () => {
      live = false;
    };
  }, []);

  const update = useCallback((patch: Partial<FeedbackDraft>) => {
    setDraft((current) => {
      const next = { ...current, ...patch };
      /*
        Written on the keystroke rather than debounced. A debounce would mean
        a window — however short — in which dismissing the form loses the last
        thing typed, and FR-005 is precisely about the words surviving a
        dismissal. The write is a `JSON.stringify` of five small fields into
        `sessionStorage`; it costs less than the render it accompanies.
      */
      saveDraft(next);
      return next;
    });
  }, []);

  const chooseKind = useCallback(
    (kind: FeedbackKind) => {
      update({ kind });
    },
    [update],
  );

  const openReview = useCallback(() => {
    // Once. Everything downstream reads this object and nothing re-reads the
    // ring — see the module header.
    setLogs(snapshot());
    setInclusions((current) => ({ ...current, logs: draft.includeLogs }));
    setStep("review");
  }, [draft.includeLogs]);

  const onCapture = useCallback(async () => {
    setCapturing(true);
    setScreenshotStatus(null);
    try {
      const outcome = await captureScreenshot();
      if (outcome.status === "captured") {
        setScreenshot(outcome.screenshot);
        setInclusions((current) => ({ ...current, screenshot: true }));
        return;
      }
      // Neither branch is an error: declining costs nothing (FR-008).
      setScreenshotStatus(
        outcome.status === "declined"
          ? "No screenshot taken. You can send this without one."
          : `${outcome.reason} You can send this without one.`,
      );
    } finally {
      setCapturing(false);
    }
  }, []);

  const toggle = useCallback(
    (part: keyof FeedbackInclusions, next: boolean) => {
      setInclusions((current) => ({ ...current, [part]: next }));
      if (part === "logs") {
        update({ includeLogs: next });
      }
    },
    [update],
  );

  const submissionDraft: FeedbackSubmissionDraft | null = useMemo(() => {
    if (!logs) {
      return null;
    }
    return {
      kind: draft.kind,
      summary: draft.summary,
      message: draft.message,
      context: collectFeedbackContext(pathname),
      logs,
      screenshot,
      inclusions,
    };
  }, [
    draft.kind,
    draft.summary,
    draft.message,
    inclusions,
    logs,
    pathname,
    screenshot,
  ]);

  const send = useCallback(async () => {
    if (!submissionDraft) {
      return;
    }
    setSubmitting(true);
    setFailure(null);
    try {
      await submitFeedback(buildFeedbackPayload(submissionDraft));
      // Only here. A draft cleared on any other path is a draft lost by a
      // failure the person did not cause.
      clearDraft();
      setDraft(loadDraft());
      setScreenshot(null);
      setStep("sent");
    } catch (error) {
      setFailure(
        isRateLimited(error)
          ? "That is a lot of feedback in a short time. Wait a few minutes and send this again — nothing you wrote has been lost."
          : containsSecret(error)
            ? "The instance found something secret in an attachment and refused it rather than sending it. Remove the logs or the screenshot and send the rest."
            : "That could not be sent right now. What you wrote is still here — try again in a moment.",
      );
    } finally {
      setSubmitting(false);
    }
  }, [submissionDraft]);

  const canContinue =
    draft.message.trim() !== "" &&
    (!definition.hasSummary || draft.summary.trim() !== "");

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        data-testid="feedback-dialog"
        className="max-h-[85vh] overflow-y-auto sm:max-w-2xl"
      >
        <DialogHeader>
          <DialogTitle>
            {step === "sent" ? "Thank you — it arrived" : "Send feedback"}
          </DialogTitle>
          <DialogDescription>
            {step === "compose"
              ? "Three kinds. Pick the one that fits and say what you need to."
              : step === "review"
                ? "Everything below is what will be sent. Remove anything you would rather keep."
                : "It is recorded here. You have not lost anything you were doing."}
          </DialogDescription>
        </DialogHeader>

        {step === "compose" ? (
          <div className="grid gap-3">
            <div
              className="grid gap-2 sm:grid-cols-3"
              role="radiogroup"
              aria-label="What kind of feedback is this?"
            >
              {FEEDBACK_KINDS.map((entry) => (
                <Button
                  key={entry.kind}
                  type="button"
                  variant={draft.kind === entry.kind ? "default" : "outline"}
                  role="radio"
                  aria-checked={draft.kind === entry.kind}
                  data-testid={`feedback-kind-${entry.testId}`}
                  className="h-auto flex-col items-start gap-1 py-2 text-left whitespace-normal"
                  onClick={() => chooseKind(entry.kind)}
                >
                  <span className="font-medium">{entry.label}</span>
                  <span className="text-xs opacity-80">{entry.hint}</span>
                </Button>
              ))}
            </div>

            {/*
              FR-003: the fields are the kind's, not a superset with some of
              them disabled. A general message has no title, so there is no
              title field to leave blank.
            */}
            {definition.hasSummary ? (
              <div className="grid gap-1">
                <Label htmlFor="feedback-summary">
                  {definition.summaryLabel}
                </Label>
                <Input
                  id="feedback-summary"
                  data-testid="feedback-summary"
                  value={draft.summary}
                  maxLength={200}
                  placeholder={definition.summaryPlaceholder}
                  onChange={(event) => update({ summary: event.target.value })}
                />
              </div>
            ) : null}

            <div className="grid gap-1">
              <Label htmlFor="feedback-message">
                {definition.messageLabel}
              </Label>
              <Textarea
                id="feedback-message"
                data-testid="feedback-message"
                rows={6}
                value={draft.message}
                placeholder={definition.messagePlaceholder}
                onChange={(event) => update({ message: event.target.value })}
              />
            </div>
          </div>
        ) : null}

        {step === "review" && submissionDraft ? (
          <FeedbackReview
            draft={submissionDraft}
            destination={destination}
            onToggle={toggle}
            onCaptureScreenshot={() => void onCapture()}
            screenshotStatus={screenshotStatus}
            capturing={capturing}
          />
        ) : null}

        {step === "sent" ? (
          <p data-testid="feedback-sent" className="text-sm">
            Your {definition.label.toLowerCase()} is recorded. It reaches the
            maintainers from here — you do not need to do anything else.
          </p>
        ) : null}

        {failure ? (
          <p
            data-testid="feedback-failure"
            role="alert"
            className="rounded-lg border border-destructive/40 bg-destructive/10 p-2 text-sm text-destructive"
          >
            {failure}
          </p>
        ) : null}

        <DialogFooter>
          {step === "compose" ? (
            <>
              <Button
                type="button"
                variant="outline"
                data-testid="feedback-cancel"
                onClick={() => onOpenChange(false)}
              >
                Not now
              </Button>
              <Button
                type="button"
                disabled={!canContinue}
                data-testid="feedback-continue"
                onClick={openReview}
              >
                Review what will be sent
              </Button>
            </>
          ) : null}
          {step === "review" ? (
            <>
              <Button
                type="button"
                variant="outline"
                data-testid="feedback-back"
                onClick={() => setStep("compose")}
              >
                Back
              </Button>
              <Button
                type="button"
                disabled={submitting}
                data-testid="feedback-submit"
                onClick={() => void send()}
              >
                {submitting ? "Sending…" : "Send it"}
              </Button>
            </>
          ) : null}
          {step === "sent" ? (
            <Button
              type="button"
              data-testid="feedback-done"
              onClick={() => onOpenChange(false)}
            >
              Back to what I was doing
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
