import type { Page, Route } from "@playwright/test";
import { expect, test } from "./fixtures/test";

/**
 * Spec 086 US6, a redirected instance: the served config names the
 * operator's collector, and the web app's records go there and nowhere else.
 * The config and the collector are both routed, so this runs on the ordinary
 * stack, which has telemetry off.
 */

interface Attr {
  key: string;
  value: { stringValue?: string; intValue?: string; boolValue?: boolean };
}
interface LogsBody {
  resourceLogs?: {
    resource: { attributes: Attr[] };
    scopeLogs: { logRecords: { attributes: Attr[] }[] }[];
  }[];
}

const attr = (attrs: Attr[], key: string) =>
  attrs.find((a) => a.key === key)?.value.stringValue;

/** Answers the collector as a real one would, preflight included. */
async function collector(page: Page, origin: string, bodies: LogsBody[]) {
  await page.route(`${origin}/**`, async (route: Route) => {
    const req = route.request();
    const cors = {
      "access-control-allow-origin": "*",
      "access-control-allow-methods": "POST",
      "access-control-allow-headers": "content-type",
    };
    if (req.method() === "POST" && new URL(req.url()).pathname === "/v1/logs") {
      bodies.push(req.postDataJSON() as LogsBody);
    }
    await route.fulfill({ status: 204, headers: cors });
  });
}

async function serveConfig(page: Page, config: Record<string, unknown>) {
  await page.route("**/telemetry.json", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      headers: { "cache-control": "no-store" },
      body: JSON.stringify(config),
    }),
  );
}

function pageViews(bodies: LogsBody[]) {
  return bodies.flatMap((b) =>
    (b.resourceLogs ?? []).flatMap((rl) =>
      rl.scopeLogs
        .flatMap((s) => s.logRecords)
        .filter((r) => attr(r.attributes, "event.name") === "page_view")
        .map((r) => ({ resource: rl.resource.attributes, record: r })),
    ),
  );
}

test.describe("Telemetry, redirected by the served config", () => {
  test.use({ storageState: { cookies: [], origins: [] } });

  test("a page view reaches the operator's collector with the operator tier and the instance id, and nothing reaches the project", async ({
    page,
  }) => {
    const instanceId = "0f3c6a1e-5b2d-4c8e-9a7f-1d2e3f4a5b6c";
    const bodies: LogsBody[] = [];
    const project: string[] = [];
    page.on("request", (r) => {
      if (new URL(r.url()).hostname.endsWith("telemetry.thunderforge.dev"))
        project.push(r.url());
    });
    await serveConfig(page, {
      enabled: true,
      endpoint: "https://otel.example.org",
      tier: "operator",
      instanceId,
      sampleRate: 1,
    });
    await collector(page, "https://otel.example.org", bodies);

    await page.goto("/login");
    await expect
      .poll(() => pageViews(bodies).length, { timeout: 15_000 })
      .toBeGreaterThan(0);

    const [view] = pageViews(bodies);
    expect(attr(view.resource, "thunderforge.tier")).toBe("operator");
    expect(attr(view.resource, "thunderforge.instance.id")).toBe(instanceId);
    expect(attr(view.resource, "service.name")).toBe("thunderforge-web");
    expect(attr(view.record.attributes, "route")).toBe("/login");
    expect(project).toEqual([]);
  });

  test("the anonymous tier says self-hosted whatever environment the config names (FR-019a)", async ({
    page,
  }) => {
    const bodies: LogsBody[] = [];
    await serveConfig(page, {
      enabled: true,
      endpoint: "https://telemetry.invalid",
      tier: "anonymous",
      environment: "staging",
      sampleRate: 1,
    });
    await collector(page, "https://telemetry.invalid", bodies);

    await page.goto("/login");
    await expect
      .poll(() => pageViews(bodies).length, { timeout: 15_000 })
      .toBeGreaterThan(0);

    const [view] = pageViews(bodies);
    expect(attr(view.resource, "thunderforge.tier")).toBe("anonymous");
    expect(attr(view.resource, "deployment.environment")).toBe("self-hosted");
  });
});
