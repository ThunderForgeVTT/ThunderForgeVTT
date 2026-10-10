import type { Page, Route } from "@playwright/test";
import { expect, test } from "./fixtures/test";

/**
 * Spec 086 US2, FR-016: an error on the page reaches the collector as one
 * record, with what is personal in it redacted, and the same error raised
 * again and again is one record with a count rather than a flood.
 *
 * The served config and the collector are both routed, so this runs on the
 * ordinary stack, which has telemetry off.
 */

const ENDPOINT = "https://telemetry.invalid";
const EMAIL = "alice.gm@example.org";

interface Attr {
  key: string;
  value: { stringValue?: string; intValue?: string | number };
}
interface LogsBody {
  resourceLogs?: {
    scopeLogs: { logRecords: { attributes: Attr[] }[] }[];
  }[];
}

const str = (attrs: Attr[], key: string) =>
  attrs.find((a) => a.key === key)?.value.stringValue;
const int = (attrs: Attr[], key: string) =>
  Number(attrs.find((a) => a.key === key)?.value.intValue ?? NaN);

async function telemetryOn(page: Page, bodies: LogsBody[]) {
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
    if (req.method() === "POST" && new URL(req.url()).pathname === "/v1/logs") {
      bodies.push(req.postDataJSON() as LogsBody);
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

function errors(bodies: LogsBody[], marker: string) {
  return bodies
    .flatMap((b) =>
      (b.resourceLogs ?? []).flatMap((rl) =>
        rl.scopeLogs.flatMap((s) => s.logRecords),
      ),
    )
    .filter((r) => str(r.attributes, "event.name") === "error")
    .filter((r) => (str(r.attributes, "error.message") ?? "").includes(marker))
    .map((r) => ({
      source: str(r.attributes, "error.source"),
      message: str(r.attributes, "error.message") ?? "",
      count: int(r.attributes, "error.count"),
    }));
}

/** Telemetry is up once the first page view has gone out. */
async function ready(page: Page, bodies: LogsBody[]) {
  await page.goto("/login");
  await expect
    .poll(
      () =>
        bodies.some((b) =>
          (b.resourceLogs ?? []).some((rl) =>
            rl.scopeLogs.some((s) =>
              s.logRecords.some(
                (r) => str(r.attributes, "event.name") === "page_view",
              ),
            ),
          ),
        ),
      { timeout: 15_000 },
    )
    .toBe(true);
}

test.describe("Telemetry, errors", () => {
  test.use({ storageState: { cookies: [], origins: [] } });

  test("an uncaught error and an unhandled rejection each post one error, with the email redacted", async ({
    page,
  }) => {
    const bodies: LogsBody[] = [];
    await telemetryOn(page, bodies);
    await ready(page, bodies);

    await page.evaluate((email) => {
      setTimeout(() => {
        throw new Error(`thrown-e2e for ${email}`);
      });
      void Promise.reject(new Error(`rejected-e2e for ${email}`));
    }, EMAIL);

    await expect
      .poll(
        () =>
          errors(bodies, "thrown-e2e").length +
          errors(bodies, "rejected-e2e").length,
        { timeout: 15_000 },
      )
      .toBe(2);

    const [thrown] = errors(bodies, "thrown-e2e");
    const [rejected] = errors(bodies, "rejected-e2e");
    expect(thrown.source).toBe("onerror");
    expect(rejected.source).toBe("unhandledrejection");
    for (const e of [thrown, rejected]) {
      expect(e.message).not.toContain(EMAIL);
      expect(e.message).toContain("[redacted");
      expect(e.count).toBe(1);
    }
  });

  test("the same error 500 times is one record with a count, and takes one of the 50 places", async ({
    page,
  }) => {
    const bodies: LogsBody[] = [];
    await telemetryOn(page, bodies);
    await ready(page, bodies);

    // Dispatched in one task, so no flush can fall between two of them and
    // the fold is the only thing under test.
    await page.evaluate(() => {
      const fire = (message: string) =>
        window.dispatchEvent(
          new ErrorEvent("error", { error: new Error(message), message }),
        );
      for (let i = 0; i < 500; i++) fire("repeated-e2e");
      for (let i = 0; i < 60; i++) fire(`distinct-e2e ${i}`);
    });

    await expect
      .poll(() => errors(bodies, "-e2e").length, { timeout: 15_000 })
      .toBe(50);

    const repeated = errors(bodies, "repeated-e2e");
    expect(repeated).toHaveLength(1);
    expect(repeated[0].count).toBe(500);
    expect(errors(bodies, "distinct-e2e")).toHaveLength(49);

    // Nothing more arrives over the next flush.
    await page.waitForTimeout(6_000);
    expect(errors(bodies, "-e2e")).toHaveLength(50);
  });
});
