/**
 * The unsent feedback draft (spec 037, FR-005).
 *
 * # Session, not local
 *
 * FR-005 says the draft survives the form being **dismissed and reopened
 * within the same session**, so `sessionStorage` is not a shortcut for
 * `localStorage` — it is the requirement. A draft that outlived the tab would
 * be an unasked-for record of what somebody was about to report, sitting on a
 * machine that may be shared. research.md § R13.
 *
 * # What is deliberately not in it
 *
 * The log snapshot and the screenshot bytes. Both are evidence, and evidence
 * is never written down (`feedbackLogBuffer.ts`'s "never persisted" property
 * would be worthless if the draft copied the buffer into storage). Reopening
 * re-reads the live buffer and re-offers the screenshot, which is also the
 * more useful behaviour: the logs at reopen include whatever went wrong since.
 *
 * # Every access is guarded
 *
 * `sessionStorage` throws on access in a browser configured to block site
 * data, and returns `null` in a fresh private window. A feedback form that
 * threw because a draft could not be read would fail at exactly the moment
 * somebody was trying to tell us something, so every path here degrades to
 * "there is no draft".
 */

import type { FeedbackKind } from "./feedbackPayload";

/** One key. A draft is singular by construction — there is one form. */
export const DRAFT_STORAGE_KEY = "thunderforge:feedback-draft";

/** What survives a dismiss: the words, the kind, and the two tick boxes. */
export interface FeedbackDraft {
  kind: FeedbackKind;
  /** The one-line title. Empty for a general message, which has none. */
  summary: string;
  message: string;
  /** Whether the person had chosen to attach the logs. */
  includeLogs: boolean;
  /** Whether they had chosen to include the context block. */
  includeContext: boolean;
}

export const EMPTY_DRAFT: FeedbackDraft = {
  kind: "ISSUE",
  summary: "",
  message: "",
  includeLogs: true,
  includeContext: true,
};

/**
 * The store, resolved per call rather than captured at module load.
 *
 * Captured once, a test could never install its own, and — more to the point
 * — a module evaluated during SSR or in a worker would throw at import time
 * instead of at use.
 */
function store(): Storage | null {
  try {
    return typeof sessionStorage === "undefined" ? null : sessionStorage;
  } catch {
    return null;
  }
}

function isKind(value: unknown): value is FeedbackKind {
  return (
    value === "ISSUE" || value === "FEATURE_REQUEST" || value === "GENERAL"
  );
}

/**
 * Read the draft, or the empty one.
 *
 * Every field is validated rather than trusted: `sessionStorage` is writable
 * by anything running on this origin, and a draft whose `kind` was some other
 * string would put the form into a state with no fields.
 */
export function loadDraft(): FeedbackDraft {
  const raw = (() => {
    try {
      return store()?.getItem(DRAFT_STORAGE_KEY) ?? null;
    } catch {
      return null;
    }
  })();

  if (raw === null) {
    return { ...EMPTY_DRAFT };
  }

  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) {
      return { ...EMPTY_DRAFT };
    }

    const candidate = parsed as Partial<FeedbackDraft>;
    return {
      kind: isKind(candidate.kind) ? candidate.kind : EMPTY_DRAFT.kind,
      summary: typeof candidate.summary === "string" ? candidate.summary : "",
      message: typeof candidate.message === "string" ? candidate.message : "",
      includeLogs:
        typeof candidate.includeLogs === "boolean"
          ? candidate.includeLogs
          : EMPTY_DRAFT.includeLogs,
      includeContext:
        typeof candidate.includeContext === "boolean"
          ? candidate.includeContext
          : EMPTY_DRAFT.includeContext,
    };
  } catch {
    return { ...EMPTY_DRAFT };
  }
}

/** Whether a draft holds anything worth restoring. */
export function draftIsEmpty(draft: FeedbackDraft): boolean {
  return draft.summary.trim() === "" && draft.message.trim() === "";
}

/**
 * Write the draft, or remove it when there is nothing in it.
 *
 * Removing rather than storing an empty object matters for the shared-machine
 * case: a person who opens the form, types nothing and closes it leaves no
 * trace at all.
 */
export function saveDraft(draft: FeedbackDraft): void {
  try {
    if (draftIsEmpty(draft)) {
      store()?.removeItem(DRAFT_STORAGE_KEY);
      return;
    }
    store()?.setItem(DRAFT_STORAGE_KEY, JSON.stringify(draft));
  } catch {
    // A quota error or a blocked store must not take the form down with it;
    // the cost is a draft that does not survive, not a submission that fails.
  }
}

/** Forget the draft. Called on a successful submission, and only then. */
export function clearDraft(): void {
  try {
    store()?.removeItem(DRAFT_STORAGE_KEY);
  } catch {
    // Same reasoning as `saveDraft`.
  }
}
