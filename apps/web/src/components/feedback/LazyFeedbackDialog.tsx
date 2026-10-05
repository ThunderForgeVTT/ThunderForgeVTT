import { lazy, type ComponentProps } from "react";
import { LazyBoundary } from "@/components/ui/lazy-boundary/LazyBoundary";
import type { FeedbackDialog } from "./FeedbackDialog";

// Spec 068 FR-001: the form, its review step, the screenshot and redaction
// code are needed by a press, not by a page load, and the launcher is in the
// entry. What has to be running before anything can throw — log capture —
// stays there (`main.tsx`).
const Dialog = lazy(() =>
  import("./FeedbackDialog").then((module) => ({
    default: module.FeedbackDialog,
  })),
);

/**
 * `FeedbackDialog`, loaded when it is opened.
 *
 * The dialog portals out of here, so the box below is empty unless the form
 * failed to arrive — in which case it holds the notice, above the launcher
 * rather than wherever the document happened to end.
 */
export function LazyFeedbackDialog(
  props: ComponentProps<typeof FeedbackDialog>,
) {
  return (
    <div className="fixed right-4 bottom-16 z-50 max-w-xs bg-background print:hidden">
      <LazyBoundary what="The feedback form" fallback={null}>
        <Dialog {...props} />
      </LazyBoundary>
    </div>
  );
}
