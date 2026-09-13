import { createElement } from "react";
import { createRoot } from "react-dom/client";

import { ImportReview, type ApprovedImport } from "@/components/import";
import type { ContentPatterns } from "@/engine/sdk/ContentPatterns";

/**
 * Mounts the review in a real page so a test can drive it as a person would.
 *
 * `book-import-review.spec.ts` proves the reading by calling the service; the
 * review is a component, and the only honest way to prove a person can exclude
 * something and submit the rest is to render it and click it. Playwright drives
 * the real DOM, so what is left here is the mounting — which needs React, and
 * React is not reachable from a bare `page.evaluate`.
 *
 * The component itself is untouched by this file's existence: it takes a file,
 * gives back entries, and knows nothing about being mounted here rather than
 * on a page. That is the point of it not being wired into one yet.
 */
export interface ReviewOutcome {
  /** What submit handed back, or null while nobody has pressed it. */
  submitted: ApprovedImport | null;
  /** How many times the window asked to be closed. */
  closes: number;
}

declare global {
  interface Window {
    importReviewOutcome?: ReviewOutcome;
  }
}

/**
 * @param pdf the document's bytes as a binary string, since only strings
 *   survive the crossing into `page.evaluate`.
 */
export function mountImportReview(
  pdf: string,
  name: string,
  patterns: ContentPatterns,
): void {
  const outcome: ReviewOutcome = { submitted: null, closes: 0 };
  window.importReviewOutcome = outcome;

  const bytes = Uint8Array.from(pdf, (character) => character.charCodeAt(0));
  const file = new File([bytes], name, { type: "application/pdf" });

  const host = document.createElement("div");
  document.body.append(host);
  createRoot(host).render(
    createElement(ImportReview, {
      file,
      patterns,
      onSubmit: (approved) => {
        outcome.submitted = approved;
      },
      onClose: () => {
        outcome.closes += 1;
      },
    }),
  );
}
