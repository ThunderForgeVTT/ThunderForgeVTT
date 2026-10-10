import { expect, test, type Page } from "@playwright/test";
import { connectSrcFor } from "@thunderforge/telemetry/vite";
import { enterPlay, openDemo, TELEMETRY_ORIGIN, WORLD_ID } from "./support";

/**
 * Spec 086 US1, SC-003: a visitor plays the demo's funnel, and each step is
 * posted once, in order, under one session.
 *
 * The preview server has telemetry off, as every other demo spec needs. This
 * spec turns it on for its own page: the served config is routed to one that
 * names `https://telemetry.invalid`, the document's `connect-src` header is
 * rewritten to what the server would send for that config, and the origin
 * itself is answered by `openDemo`'s route with a 204. The browser enforces
 * a real header, and nothing leaves the machine.
 *
 * The funnel's first step, `landing_viewed`, is the landing's to send, and
 * `apps/landing/e2e/telemetry.spec.ts` proves it. In production the two
 * share the session through `sessionStorage` on one origin.
 */

const CONFIG = {
  enabled: true,
  endpoint: TELEMETRY_ORIGIN,
  sampleRate: 1,
  environment: "production",
  tier: "anonymous",
};

const DEMO_STEPS = [
  "demo_opened",
  "map_loaded",
  "token_moved",
  "dice_rolled",
  "view_switched",
  "cta_clicked",
];

interface Attr {
  key: string;
  value: { stringValue?: string; intValue?: string };
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
        s.logRecords.map((r) => ({
          resource: rl.resource.attributes,
          attributes: r.attributes,
        })),
      ),
    );
  });
}

function funnel(bodies: string[]) {
  return records(bodies)
    .filter((r) => attr(r.attributes, "event.name") === "funnel")
    .map((r) => ({
      step: attr(r.attributes, "step"),
      session: attr(r.attributes, "session.id"),
      entry: attr(r.attributes, "entry"),
      cta: attr(r.attributes, "cta"),
      service: attr(r.resource, "service.name"),
    }));
}

/** Telemetry on, for this page only. */
async function turnOn(page: Page) {
  await page.route("**/telemetry.json", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      headers: { "cache-control": "no-store" },
      body: JSON.stringify(CONFIG),
    }),
  );
  await page.route("**/demo/**", async (route) => {
    if (route.request().resourceType() !== "document") {
      await route.fallback();
      return;
    }
    const response = await route.fetch();
    await route.fulfill({
      response,
      headers: {
        ...response.headers(),
        "content-security-policy": connectSrcFor(CONFIG as never),
      },
    });
  });
}

async function toScreen(page: Page, point: { x: number; y: number }) {
  const box = await page.locator("canvas").boundingBox();
  if (!box) throw new Error("the board's canvas is not on the page");
  return {
    x: box.x + box.width / 2 + point.x,
    y: box.y + box.height / 2 - point.y,
  };
}

async function firstToken(page: Page): Promise<{ x: number; y: number }> {
  return page.evaluate(async (worldId) => {
    const ask = async (query: string, variables: object) =>
      (
        await (
          await fetch("/api/graphql", {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ query, variables }),
          })
        ).json()
      ).data;
    const { world } = await ask(
      "query ($id: UUID!) { world(id: $id) { activeSceneId } }",
      { id: worldId },
    );
    const { tokens } = await ask(
      "query ($s: UUID!) { tokens(sceneId: $s) { x y } }",
      { s: world.activeSceneId },
    );
    return tokens[0];
  }, WORLD_ID);
}

test("a visitor's run through the demo posts each funnel step once, in order, under one session", async ({
  browser,
  baseURL,
}) => {
  const { page, outside, telemetry } = await openDemo(browser, baseURL);
  await turnOn(page);
  const origin = new URL(baseURL as string).origin;

  // Opened from the landing's link.
  await page.goto("/demo/", { referer: `${origin}/` });
  await expect(page.getByTestId("demo-notice")).toBeVisible();
  // The step is posted from this page before the visitor moves on; the
  // next `goto` is a full load, which a keepalive post racing the unload
  // would make this spec flaky over.
  await expect
    .poll(() => funnel(telemetry).map((f) => f.step), { timeout: 30_000 })
    .toEqual(["demo_opened"]);

  // The map on the board.
  await enterPlay(page);

  // A token dragged. The engine takes a while to take a pointer after the
  // canvas appears, so a drag that moved nothing is made again.
  const before = await firstToken(page);
  await expect(async () => {
    const start = await toScreen(page, before);
    const end = await toScreen(page, { x: before.x + 234, y: before.y });
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.mouse.move(end.x, end.y, { steps: 14 });
    await page.waitForTimeout(150);
    await page.mouse.up();
    await expect
      .poll(async () => (await firstToken(page)).x, { timeout: 3_000 })
      .not.toBe(before.x);
  }).toPass({ timeout: 90_000 });

  // A roll.
  await page.getByTestId("dice-formula-input").fill("1d20");
  await page.getByTestId("dice-roll-button").click();
  await expect(page.getByTestId("dice-roll-result")).toBeVisible({
    timeout: 30_000,
  });

  // The view switched, which reloads the page.
  await page.getByRole("button", { name: "View as player" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText("a player", {
    timeout: 30_000,
  });

  // **Run your own**. The landing is not served here, so the click is kept
  // from navigating; the link's own handler has run by then.
  const cta = page.getByTestId("demo-run-your-own");
  await expect(cta).toHaveAttribute("href", "/#self-host");
  await cta.evaluate((a) =>
    a.addEventListener("click", (e) => e.preventDefault()),
  );
  await cta.click();

  await expect
    .poll(() => funnel(telemetry).map((f) => f.step), { timeout: 30_000 })
    .toEqual(DEMO_STEPS);

  const steps = funnel(telemetry);
  expect(new Set(steps.map((s) => s.session)).size).toBe(1);
  expect(steps[0].session).toMatch(/^[0-9a-f]{32}$/);
  expect(steps.every((s) => s.service === "thunderforge-demo")).toBe(true);
  expect(steps[0].entry).toBe("landing");
  expect(steps[5].cta).toBe("run_your_own");

  // The actions were counted by kind, and nothing else went out.
  const actions = records(telemetry)
    .filter((r) => attr(r.attributes, "event.name") === "demo.action")
    .map((r) => attr(r.attributes, "action"));
  expect(actions).toEqual(
    expect.arrayContaining(["token_moved", "dice_rolled", "view_switched"]),
  );
  expect(outside, "only telemetry leaves the demo's own files").toEqual([]);

  await page.context().close();
});

test("with the served config off, the demo posts nothing at all", async ({
  browser,
  baseURL,
}) => {
  const { page, outside, telemetry } = await openDemo(browser, baseURL);
  const chunks: string[] = [];
  page.on("request", (r) => {
    if (/telemetryChunk/.test(r.url())) chunks.push(r.url());
  });
  await page.goto("/demo/");
  await expect(page.getByTestId("demo-notice")).toBeVisible();
  await page.getByRole("button", { name: "View as player" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText("a player");
  await page.waitForTimeout(6_000); // past one flush interval
  expect(telemetry).toEqual([]);
  expect(chunks, "the telemetry chunk is never fetched").toEqual([]);
  expect(outside).toEqual([]);
  await page.context().close();
});

test("a thrown error is posted redacted, with a stack of path and line only", async ({
  browser,
  baseURL,
}) => {
  const { page, outside, telemetry } = await openDemo(browser, baseURL);
  await turnOn(page);
  await page.goto("/demo/");
  await expect(page.getByTestId("demo-notice")).toBeVisible();
  await expect
    .poll(() => funnel(telemetry).map((f) => f.step), { timeout: 30_000 })
    .toContain("demo_opened");

  await page.evaluate(() => {
    setTimeout(() => {
      throw new Error("demo-e2e broke for bryn@example.org");
    });
  });

  const posted = () =>
    records(telemetry)
      .filter((r) => attr(r.attributes, "event.name") === "error")
      .filter((r) =>
        (attr(r.attributes, "error.message") ?? "").includes("demo-e2e"),
      );
  await expect.poll(() => posted().length, { timeout: 30_000 }).toBe(1);

  const [error] = posted();
  const message = attr(error.attributes, "error.message") ?? "";
  const stack = attr(error.attributes, "error.stack") ?? "";
  expect(attr(error.resource, "service.name")).toBe("thunderforge-demo");
  expect(attr(error.attributes, "error.source")).toBe("onerror");
  expect(message).not.toContain("bryn@example.org");
  expect(message).toContain("[redacted");
  // Each frame is `path:line`: no origin, no query, no column.
  expect(stack.length).toBeGreaterThan(0);
  for (const frame of stack.split("\n")) {
    expect(frame).toMatch(/^[^\s?#]*:\d+$/);
    expect(frame).not.toMatch(/^https?:/);
  }
  expect(outside).toEqual([]);
  await page.context().close();
});
