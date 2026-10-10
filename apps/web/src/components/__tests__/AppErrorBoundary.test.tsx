/**
 * Spec 086 US2, SC-006: a render that throws shows the boundary's page, and
 * the boundary posts one `error` event, redacted.
 *
 * The web suite runs in Node, with no DOM, and React's server renderer does
 * not run `getDerivedStateFromError` (see `PackSurfaceBoundary.tsx`). So the
 * boundary's three steps are driven as React drives them: the state a throw
 * gives, the page `render` returns in that state, and `componentDidCatch`'s
 * report. The browser half is `apps/web/e2e/telemetry-errors.spec.ts`.
 */
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  createTelemetry,
  memoryStorage,
  type Batch,
} from "@thunderforge/telemetry";
import { redact } from "../../services/feedbackRedaction";
import { AppErrorBoundary } from "../AppErrorBoundary";

function fakeTelemetry() {
  const batches: Batch[] = [];
  const t = createTelemetry({
    service: "thunderforge-web",
    version: "test",
    config: {
      enabled: true,
      endpoint: "https://telemetry.invalid",
      sampleRate: 1,
      environment: "production",
      tier: "anonymous",
    },
    sink: {
      send: async (batch) => {
        batches.push(batch);
      },
    },
    redact: (text) => redact(text).text,
    storage: memoryStorage(),
    now: () => 1_000,
    random: () => 0.5,
    privacy: { gpc: false, dnt: false },
  });
  return { t, batches };
}

const Throws = () => {
  throw new Error("render failed for canary-7f3a@example.org");
};

describe("AppErrorBoundary", () => {
  it("renders its children until something throws", () => {
    const html = renderToStaticMarkup(
      <AppErrorBoundary>
        <p>the app</p>
      </AppErrorBoundary>,
    );
    expect(html).toBe("<p>the app</p>");
  });

  it("shows its page after a throw, and posts one redacted error", async () => {
    const { t, batches } = fakeTelemetry();
    const boundary = new AppErrorBoundary({
      children: <Throws />,
      report: (error) => t.error("boundary", error),
    });
    const error = new Error("render failed for canary-7f3a@example.org");

    boundary.state = AppErrorBoundary.getDerivedStateFromError(error);
    const page = renderToStaticMarkup(<>{boundary.render()}</>);
    expect(page).toContain('data-testid="app-error"');
    expect(page).toContain("Reload");
    expect(page).not.toContain("canary-7f3a");

    boundary.componentDidCatch(error, { componentStack: "\n    at Throws" });
    t.flush(false);
    await Promise.resolve();

    const errors = batches
      .flatMap((b) => b.records)
      .filter((r) => r.kind === "log" && r.name === "error");
    expect(errors).toHaveLength(1);
    const attrs = errors[0].attrs;
    expect(attrs["error.source"]).toBe("boundary");
    expect(String(attrs["error.message"])).not.toContain("canary-7f3a");
    expect(String(attrs["error.message"])).toContain("[redacted");
  });

  it("reports through nothing when no reporter is loaded", () => {
    // The default reporter is the package's facade, which drops an error
    // until the telemetry chunk has arrived; it must not throw.
    const boundary = new AppErrorBoundary({ children: null });
    expect(() =>
      boundary.componentDidCatch(new Error("x"), { componentStack: "" }),
    ).not.toThrow();
  });
});
