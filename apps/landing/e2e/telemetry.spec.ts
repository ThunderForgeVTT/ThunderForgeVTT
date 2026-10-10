import { expect, test, type Page } from "@playwright/test";
import { connectSrcFor } from "@thunderforge/telemetry/vite";

/**
 * Spec 086 US1: the landing sends `landing_viewed`, the funnel's first step,
 * and a `cta_clicked` event for each call to action, with where it was.
 *
 * The preview server has telemetry off. This spec turns it on for its own
 * page, as `apps/demo/e2e/telemetry.spec.ts` does: the served config names
 * `https://telemetry.invalid`, the document's `connect-src` is what nginx
 * would send for that config, and the origin is answered here with a 204.
 */

const TELEMETRY_ORIGIN = "https://telemetry.invalid";
const CONFIG = {
  enabled: true,
  endpoint: TELEMETRY_ORIGIN,
  sampleRate: 1,
  environment: "production",
  tier: "anonymous",
};

/** Every call to action on the page, as the dashboard groups them. */
const CTAS = [
  "github nav",
  "github hero",
  "github star_chart",
  "github support",
  "sponsor nav",
  "sponsor hero",
  "sponsor support",
  "try_demo hero",
  "try_demo map_legend",
].sort();

interface Attr {
  key: string;
  value: { stringValue?: string };
}
interface LogsBody {
  resourceLogs?: {
    resource: { attributes: Attr[] };
    scopeLogs: { logRecords: { attributes: Attr[] }[] }[];
  }[];
}
const attr = (attrs: Attr[], key: string) =>
  attrs.find((a) => a.key === key)?.value.stringValue;

function records(bodies: string[]) {
  return bodies.flatMap((raw) => {
    let body: LogsBody;
    try {
      body = JSON.parse(raw) as LogsBody;
    } catch {
      return [];
    }
    return (body.resourceLogs ?? []).flatMap((rl) =>
      rl.scopeLogs.flatMap((s) =>
        s.logRecords.map((r) => ({ resource: rl.resource.attributes, attributes: r.attributes })),
      ),
    );
  });
}
const named = (bodies: string[], name: string) =>
  records(bodies).filter((r) => attr(r.attributes, "event.name") === name);

async function open(page: Page, config: object | null, path = "/") {
  const telemetry: string[] = [];
  const outside: string[] = [];
  const origin = new URL(test.info().project.use.baseURL as string).origin;
  await page.route(`${TELEMETRY_ORIGIN}/**`, async (route) => {
    const body = route.request().postData();
    if (body) telemetry.push(body);
    await route.fulfill({
      status: 204,
      headers: {
        "access-control-allow-origin": origin,
        "access-control-allow-methods": "POST",
        "access-control-allow-headers": "content-type",
      },
    });
  });
  // GitHub's numbers are not this spec's business.
  await page.route("**/gh/**", (route) => route.fulfill({ status: 404, body: "" }));
  if (config) {
    await page.route("**/telemetry.json", (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        headers: { "cache-control": "no-store" },
        body: JSON.stringify(config),
      }),
    );
    await page.route((url) => url.origin === origin && url.pathname === "/", async (route) => {
      const response = await route.fetch();
      await route.fulfill({
        response,
        headers: {
          ...response.headers(),
          "content-security-policy": connectSrcFor(config as never),
        },
      });
    });
  }
  page.on("request", (request) => {
    const url = new URL(request.url());
    if (url.origin === origin || url.protocol === "data:" || url.protocol === "blob:") return;
    if (url.origin === TELEMETRY_ORIGIN && /^\/v1\/(logs|traces)$/.test(url.pathname)) return;
    outside.push(`${request.method()} ${request.url()}`);
  });
  // The links stay on the page, so every one can be clicked in one visit.
  await page.addInitScript(() =>
    window.addEventListener("click", (e) => {
      if ((e.target as Element | null)?.closest?.("a[data-cta]")) e.preventDefault();
    }),
  );
  await page.goto(path);
  return { telemetry, outside };
}

test("the landing sends landing_viewed, and each call to action with its place", async ({
  page,
}) => {
  const { telemetry, outside } = await open(page, CONFIG);

  await expect
    .poll(() => named(telemetry, "funnel").map((r) => attr(r.attributes, "step")), {
      timeout: 30_000,
    })
    .toEqual(["landing_viewed"]);
  const [viewed] = named(telemetry, "funnel");
  expect(attr(viewed.resource, "service.name")).toBe("thunderforge-landing");
  expect(attr(viewed.attributes, "session.id")).toMatch(/^[0-9a-f]{32}$/);
  expect(named(telemetry, "page_view").map((r) => attr(r.attributes, "route"))).toEqual(["/"]);

  const links = page.locator("a[data-cta]");
  const tagged = await links.evaluateAll((as) =>
    as.map((a) => `${(a as HTMLElement).dataset.cta} ${(a as HTMLElement).dataset.placement}`),
  );
  expect([...tagged].sort()).toEqual(CTAS);
  for (let i = 0; i < tagged.length; i += 1) await links.nth(i).click();

  await expect
    .poll(
      () =>
        named(telemetry, "cta_clicked")
          .map((r) => `${attr(r.attributes, "cta")} ${attr(r.attributes, "placement")}`)
          .sort(),
      { timeout: 30_000 },
    )
    .toEqual(CTAS);
  // A landing click is not the funnel's last step; the demo's is.
  expect(named(telemetry, "funnel")).toHaveLength(1);
  expect(outside, "only telemetry leaves the page").toEqual([]);
});

test("with the served config off, the landing posts nothing and loads no chunk", async ({
  page,
}) => {
  const chunks: string[] = [];
  page.on("request", (r) => {
    if (/telemetryChunk/.test(r.url())) chunks.push(r.url());
  });
  const { telemetry, outside } = await open(page, null);
  await expect(page.locator("a[data-cta]").first()).toBeVisible();
  await page.locator("a[data-cta]").first().click();
  await page.waitForTimeout(6_000); // past one flush interval
  expect(telemetry).toEqual([]);
  expect(chunks).toEqual([]);
  expect(outside).toEqual([]);
});

test("the page view carries the campaign tags, and nothing else from the address", async ({
  page,
}) => {
  const { telemetry, outside } = await open(
    page,
    CONFIG,
    "/?utm_source=newsletter&utm_medium=email&utm_campaign=launch%20me%40example.org&ref=secret",
  );
  await expect
    .poll(() => named(telemetry, "page_view").length, { timeout: 30_000 })
    .toBe(1);
  const [view] = named(telemetry, "page_view");
  expect(attr(view.attributes, "route")).toBe("/");
  expect(attr(view.attributes, "utm.source")).toBe("newsletter");
  expect(attr(view.attributes, "utm.medium")).toBe("email");
  // Only letters, digits, `.`, `_` and `-` survive in a tag.
  expect(attr(view.attributes, "utm.campaign")).toBe("launchmeexample.org");
  expect(JSON.stringify(view.attributes)).not.toContain("secret");
  expect(outside).toEqual([]);
});

test("scroll depth is the deepest section reached, sent once when the page is left", async ({
  page,
}) => {
  const { telemetry, outside } = await open(page, CONFIG);
  await expect
    .poll(() => named(telemetry, "page_view").length, { timeout: 30_000 })
    .toBe(1);

  // Down the page a section at a time, as a reader would.
  const sections = page.locator("[data-section]");
  const names = await sections.evaluateAll((els) =>
    els.map((el) => (el as HTMLElement).dataset.section),
  );
  expect(names).toEqual([
    "hero",
    "dream",
    "dice",
    "map",
    "numbers",
    "self-host",
    "telemetry",
    "stance",
    "stars",
    "support",
    "footer",
  ]);
  for (let i = 0; i < names.length; i += 1) {
    await sections.nth(i).scrollIntoViewIfNeeded();
    await page.waitForTimeout(100);
  }
  // Back to the top: depth is the deepest reached, not where the reader is.
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.waitForTimeout(100);

  await page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide")));
  await page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide")));
  await expect
    .poll(() => named(telemetry, "scroll_depth").map((r) => attr(r.attributes, "section")), {
      timeout: 30_000,
    })
    .toEqual(["footer"]);
  expect(outside).toEqual([]);
});

test("What we measure says what is collected, and the footer links to it", async ({ page }) => {
  await open(page, null);
  const section = page.locator("#telemetry");
  await expect(section.getByRole("heading", { level: 2 })).toHaveText("What we measure.");
  await expect(section).toContainText("We count what happens, not what you say.");
  await expect(section).toContainText("we receive errors only.");
  await expect(section).toContainText("turn them off with TELEMETRY=false.");
  await expect(section.getByRole("link", { name: "How" })).toHaveAttribute(
    "href",
    /docs\/guides\/telemetry\.md$/,
  );
  await expect(page.locator("footer a[href='#telemetry']")).toHaveText("What we measure");
});

test("with Global Privacy Control, the landing sends errors only", async ({ page }) => {
  await page.addInitScript(() =>
    Object.defineProperty(Navigator.prototype, "globalPrivacyControl", {
      get: () => true,
      configurable: true,
    }),
  );
  const { telemetry, outside } = await open(page, CONFIG);
  await expect(page.locator("#telemetry")).toBeAttached();
  // A visit that would otherwise send a page view, a click and a depth.
  await page.locator("[data-cta]").first().click();
  await page.locator("footer").scrollIntoViewIfNeeded();
  await page.evaluate(() => {
    setTimeout(() => {
      throw new Error("landing-e2e gpc");
    });
  });
  await page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide")));
  await expect.poll(() => named(telemetry, "error").length, { timeout: 30_000 }).toBe(1);
  await page.waitForTimeout(6_000); // past one flush interval
  expect([...new Set(records(telemetry).map((r) => attr(r.attributes, "event.name")))]).toEqual([
    "error",
  ]);
  expect(outside).toEqual([]);
});
