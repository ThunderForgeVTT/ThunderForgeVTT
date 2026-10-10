/**
 * The last boundary: a page that threw while rendering (spec 086 US2,
 * FR-016).
 *
 * `PackSurfaceBoundary` contains a pack's surface, so the session goes on.
 * Anything else that throws unmounts the whole tree, and the visitor saw a
 * blank page that nobody heard about. This catches it at the routes, says so
 * plainly with a way back, and reports it.
 *
 * It reports through the package's facade, which is the telemetry chunk's
 * reporter once that has loaded and drops the call before then, or when
 * telemetry is off. The message never names the error: what it says may be
 * the visitor's own text, and the report redacts it, the page cannot.
 */
import { Component, type ErrorInfo, type ReactNode } from "react";
import { telemetry } from "@thunderforge/telemetry";
import { Button } from "@/components/ui/button/Button";

interface Props {
  children: ReactNode;
  /** Where the error goes. The facade by default; a test passes its own. */
  report?: (error: unknown) => void;
}

interface State {
  failed: boolean;
}

export class AppErrorBoundary extends Component<Props, State> {
  state: State = { failed: false };

  static getDerivedStateFromError(_error: unknown): State {
    return { failed: true };
  }

  componentDidCatch(error: unknown, _info: ErrorInfo): void {
    try {
      (this.props.report ?? ((e) => telemetry.error("boundary", e)))(error);
    } catch {
      // A report that fails is never a second failure for the visitor.
    }
  }

  render(): ReactNode {
    if (!this.state.failed) return this.props.children;
    return (
      <main
        role="alert"
        data-testid="app-error"
        className="grid min-h-[60vh] place-items-center p-6 text-center"
      >
        <div className="grid max-w-md gap-3">
          <h1 className="text-2xl font-semibold">
            Something went wrong on this page.
          </h1>
          <p className="text-muted-foreground">
            Your world is saved on the server. Reloading usually brings the page
            back.
          </p>
          <div className="flex justify-center">
            <Button onClick={() => location.reload()}>Reload</Button>
          </div>
        </div>
      </main>
    );
  }
}
