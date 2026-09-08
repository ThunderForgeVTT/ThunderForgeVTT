/**
 * The three kinds, and what each one asks for (spec 037, FR-002 and FR-003).
 *
 * FR-003 is the reason this is data rather than three branches in the form:
 * "the form MUST ask for what its kind needs and MUST NOT require fields that
 * do not apply". A general message has no title because a general message is
 * not a work item; an issue asks what happened and a feature request asks what
 * is wanted, and the difference is in the words the person is prompted with,
 * not only in a label on the eventual issue.
 */

import type { FeedbackKind } from "@/services/feedbackPayload";

export interface FeedbackKindDefinition {
  kind: FeedbackKind;
  /** The control's label, and how the review names it back. */
  label: string;
  /** Why somebody would pick this one, in one line. */
  hint: string;
  /**
   * Whether this kind has a one-line title.
   *
   * `GENERAL` does not — `contracts/feedback.md` declares `summary` "required
   * for ISSUE and FEATURE_REQUEST, absent for GENERAL", and a form that asked
   * for one anyway would be demanding a field that does not apply.
   */
  hasSummary: boolean;
  summaryLabel: string;
  summaryPlaceholder: string;
  messageLabel: string;
  messagePlaceholder: string;
  /** For `data-testid`, so the suite names a kind the way a person does. */
  testId: string;
}

export const FEEDBACK_KINDS: FeedbackKindDefinition[] = [
  {
    kind: "ISSUE",
    label: "Something is broken",
    hint: "A bug, an error, something that did not do what it should.",
    hasSummary: true,
    summaryLabel: "What went wrong, in one line",
    summaryPlaceholder: "Tokens stop moving after a scene change",
    messageLabel: "What happened?",
    messagePlaceholder:
      "What you were doing, what you expected, and what happened instead.",
    testId: "issue",
  },
  {
    kind: "FEATURE_REQUEST",
    label: "I want something",
    hint: "Something that does not exist yet, or does not work the way you need.",
    hasSummary: true,
    summaryLabel: "What you want, in one line",
    summaryPlaceholder: "Let a GM reorder initiative by dragging",
    messageLabel: "What would you like?",
    messagePlaceholder:
      "What you are trying to do, and what would make it possible.",
    testId: "feature-request",
  },
  {
    kind: "GENERAL",
    label: "Something else",
    hint: "A question, a thought, or anything that is neither of the above.",
    hasSummary: false,
    summaryLabel: "",
    summaryPlaceholder: "",
    messageLabel: "Your message",
    messagePlaceholder: "Tell us anything.",
    testId: "general",
  },
];

export function definitionFor(kind: FeedbackKind): FeedbackKindDefinition {
  // The list is exhaustive over the union, so the fallback is unreachable —
  // it exists so a fourth kind added to the type without a definition is a
  // rendering oddity rather than a crash on somebody's bug report.
  return (
    FEEDBACK_KINDS.find((entry) => entry.kind === kind) ?? FEEDBACK_KINDS[0]
  );
}
