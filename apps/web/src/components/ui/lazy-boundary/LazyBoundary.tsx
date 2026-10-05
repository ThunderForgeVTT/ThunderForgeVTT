import { Component, Suspense, type ReactNode } from "react";
import { Button } from "@/components/ui/button/Button";
import { isChunkLoadError } from "./chunkLoadError";

export interface LazyBoundaryProps {
  /** What stands in while the code arrives. */
  fallback: ReactNode;
  /** What is being loaded, as the sentence names it: "The editor". */
  what: string;
  children: ReactNode;
}

/**
 * Where a part of a page that is loaded on demand sits (spec 068 FR-007).
 *
 * `Suspense` covers the wait. This covers the other outcome: the code never
 * arrives, most often because a deploy replaced it while the tab was open.
 * Without a boundary that unmounts the whole app; with one, the page stays
 * and says what is missing.
 *
 * Only a failed download is caught. Anything else a child throws is thrown
 * again, so a bug is not reported to somebody as their connection.
 */
export class LazyBoundary extends Component<
  LazyBoundaryProps,
  { error: unknown }
> {
  state: { error: unknown } = { error: null };

  static getDerivedStateFromError(error: unknown) {
    return { error };
  }

  render() {
    const { error } = this.state;
    if (error !== null) {
      if (!isChunkLoadError(error)) {
        throw error;
      }
      return <NotLoaded what={this.props.what} />;
    }
    return (
      <Suspense fallback={this.props.fallback}>{this.props.children}</Suspense>
    );
  }
}

/** What a boundary shows when the code did not arrive. */
export function NotLoaded({ what }: { what: string }) {
  return (
    <div
      role="alert"
      className="grid justify-items-start gap-2 rounded-md border border-border p-3 text-sm"
      data-testid="lazy-not-loaded"
    >
      <p>
        {what} did not load. The app may have been updated since this page was
        opened.
      </p>
      <Button
        type="button"
        variant="secondary"
        onClick={() => window.location.reload()}
      >
        Reload
      </Button>
    </div>
  );
}
