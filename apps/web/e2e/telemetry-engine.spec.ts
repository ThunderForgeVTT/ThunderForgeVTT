import type { Page, Route } from "@playwright/test";
import { expect, test } from "./fixtures/test";
import {
  createWorldAndPlay,
  register,
  waitForEngineReady,
} from "./fixtures/offline";

/**
 * Spec 086 US7: a sampled session's board load is one `engine.load` trace
 * with its three stages, and a GraphQL request names a browser span in its
 * `traceparent`, so the server's span joins it.
 *
 * The served config and the collector are both routed, so this runs on the
 * ordinary stack, which has telemetry off.
 */

const ENDPOINT = "https://telemetry.invalid";

interface Span {
  traceId: string;
  spanId: string;
  parentSpanId?: string;
  name: string;
  startTimeUnixNano: string;
  endTimeUnixNano: string;
}
interface TracesBody {
  resourceSpans?: { scopeSpans: { spans: Span[] }[] }[];
}

async function telemetryOn(page: Page, bodies: TracesBody[]) {
  await page.route("**/telemetry.json", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      headers: { "cache-control": "no-store" },
      body: JSON.stringify({
        enabled: true,
        endpoint: ENDPOINT,
        tier: "anonymous",
        sampleRate: 1,
      }),
    }),
  );
  await page.route(`${ENDPOINT}/**`, async (route: Route) => {
    const req = route.request();
    if (
      req.method() === "POST" &&
      new URL(req.url()).pathname === "/v1/traces"
    ) {
      bodies.push(req.postDataJSON() as TracesBody);
    }
    await route.fulfill({
      status: 204,
      headers: {
        "access-control-allow-origin": "*",
        "access-control-allow-methods": "POST",
        "access-control-allow-headers": "content-type",
      },
    });
  });
}

const spans = (bodies: TracesBody[]) =>
  bodies.flatMap((b) =>
    (b.resourceSpans ?? []).flatMap((rs) =>
      rs.scopeSpans.flatMap((s) => s.spans),
    ),
  );

/** Everything queued goes out now. */
const flush = (page: Page) =>
  page.evaluate(() =>
    window.dispatchEvent(new PageTransitionEvent("pagehide")),
  );

test.describe("Telemetry, the engine and requests", () => {
  test.use({ storageState: { cookies: [], origins: [] } });

  test("mounting the board posts an engine.load trace with three stages, and a request's traceparent names a posted span", async ({
    page,
  }) => {
    test.setTimeout(4 * 60_000);
    const bodies: TracesBody[] = [];
    await telemetryOn(page, bodies);
    const traceparents: string[] = [];
    page.on("request", (req) => {
      if (!new URL(req.url()).pathname.endsWith("/graphql")) return;
      const tp = req.headers()["traceparent"];
      if (tp) traceparents.push(tp);
    });

    await register(page, "tracer");
    await createWorldAndPlay(page, `E2E Traced ${Date.now()}`);
    await waitForEngineReady(page);

    await expect
      .poll(
        async () => {
          await flush(page);
          return spans(bodies).some((s) => s.name === "engine.load");
        },
        { timeout: 60_000, intervals: [2_000] },
      )
      .toBe(true);

    const all = spans(bodies);
    const [root] = all.filter((s) => s.name === "engine.load");
    expect(root.parentSpanId ?? "").toBe("");
    const children = all
      .filter((s) => s.parentSpanId === root.spanId)
      .map((s) => s.name)
      .sort();
    expect(children).toEqual(["compile", "download", "start"]);
    for (const child of all.filter((s) => s.parentSpanId === root.spanId)) {
      expect(child.traceId).toBe(root.traceId);
      expect(BigInt(child.startTimeUnixNano)).toBeGreaterThanOrEqual(
        BigInt(root.startTimeUnixNano),
      );
      expect(BigInt(child.endTimeUnixNano)).toBeLessThanOrEqual(
        BigInt(root.endTimeUnixNano),
      );
    }

    // A request carried a traceparent, and its span was posted.
    expect(traceparents.length).toBeGreaterThan(0);
    for (const tp of traceparents) {
      expect(tp).toMatch(/^00-[0-9a-f]{32}-[0-9a-f]{16}-01$/);
    }
    await expect
      .poll(
        async () => {
          await flush(page);
          const posted = spans(bodies).filter(
            (s) => s.name === "graphql.request",
          );
          return traceparents.some((tp) => {
            const [, traceId, spanId] = tp.split("-");
            return posted.some(
              (s) => s.traceId === traceId && s.spanId === spanId,
            );
          });
        },
        { timeout: 30_000, intervals: [2_000] },
      )
      .toBe(true);
  });

  test("an unsampled session sends no traceparent", async ({ page }) => {
    await page.route("**/telemetry.json", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        headers: { "cache-control": "no-store" },
        body: JSON.stringify({
          enabled: true,
          endpoint: ENDPOINT,
          tier: "anonymous",
          sampleRate: 0,
        }),
      }),
    );
    await page.route(`${ENDPOINT}/**`, (route) =>
      route.fulfill({
        status: 204,
        headers: { "access-control-allow-origin": "*" },
      }),
    );
    const traced: string[] = [];
    const sent: string[] = [];
    page.on("request", (req) => {
      if (!new URL(req.url()).pathname.endsWith("/graphql")) return;
      sent.push(req.url());
      if (req.headers()["traceparent"]) traced.push(req.url());
    });
    await register(page, "untraced");
    await page.goto("/worlds");
    await expect
      .poll(() => sent.length, { timeout: 30_000 })
      .toBeGreaterThan(0);
    expect(traced).toEqual([]);
  });
});
