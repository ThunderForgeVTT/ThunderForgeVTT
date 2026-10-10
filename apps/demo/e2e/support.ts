import {
  expect,
  type Browser,
  type BrowserContext,
  type Page,
} from "@playwright/test";

/**
 * What the demo's feature specs share: a page whose every request outside
 * the demo's own files is logged, and the backend asked the way the app asks
 * it. `demo.spec.ts` keeps its own copies; it predates this file.
 */

export const WORLD_ID = "d0000000-0000-4000-0002-000000000001";

/**
 * Spec 086 FR-033: where a test's telemetry goes. Nothing answers there; the
 * context does, with a 204, so nothing leaves the machine.
 */
export const TELEMETRY_ORIGIN = "https://telemetry.invalid";
const TELEMETRY_PATHS = new Set(["/v1/logs", "/v1/traces"]);

export interface Opened {
  page: Page;
  /**
   * Every request and socket that is not one of the demo's own files, nor a
   * telemetry post to the configured origin.
   */
  outside: string[];
  /** The body of every telemetry post, in the order they were sent. */
  telemetry: string[];
}

/** Whether a request is a telemetry post to the test's origin. */
export function isTelemetry(url: URL): boolean {
  return url.origin === TELEMETRY_ORIGIN && TELEMETRY_PATHS.has(url.pathname);
}

/**
 * Answers the telemetry origin as a collector would, preflight included, and
 * keeps each post's body.
 */
export async function routeTelemetry(
  context: BrowserContext,
  telemetry: string[],
): Promise<void> {
  await context.route(`${TELEMETRY_ORIGIN}/**`, async (route) => {
    const request = route.request();
    if (request.method() === "POST") telemetry.push(request.postData() ?? "");
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

export async function openDemo(
  browser: Browser,
  baseURL: string | undefined,
): Promise<Opened> {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
  });
  const page = await context.newPage();
  const outside: string[] = [];
  const telemetry: string[] = [];
  await routeTelemetry(context, telemetry);
  const origin = new URL(baseURL as string).origin;
  context.on("request", (request) => {
    const url = new URL(request.url());
    if (url.protocol === "blob:" || url.protocol === "data:") return;
    if (url.origin === origin && url.pathname.startsWith("/demo/")) return;
    if (isTelemetry(url)) return;
    outside.push(`${request.method()} ${request.url()}`);
  });
  page.on("websocket", (socket) => outside.push(`SOCKET ${socket.url()}`));
  return { page, outside, telemetry };
}

export type Answer<T> = {
  status: number;
  body: {
    data?: T;
    errors?: { message: string; extensions?: { code?: string } }[];
  };
};

/** Ask the demo's own backend, the way the app does. */
export async function ask<T>(
  page: Page,
  query: string,
  variables: object = {},
): Promise<Answer<T>> {
  return page.evaluate(
    async ([query, variables]) => {
      const response = await fetch("/api/graphql", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ query, variables }),
      });
      return { status: response.status, body: await response.json() };
    },
    [query, variables] as const,
  ) as Promise<Answer<T>>;
}

/** `ask`, and the data, or the errors thrown. */
export async function data<T>(
  page: Page,
  query: string,
  variables: object = {},
): Promise<T> {
  const answer = await ask<T>(page, query, variables);
  if (!answer.body.data || answer.body.errors) {
    throw new Error(JSON.stringify(answer.body.errors));
  }
  return answer.body.data;
}

export async function activeSceneId(page: Page): Promise<string> {
  const { world } = await data<{ world: { activeSceneId: string } }>(
    page,
    "query ($id: UUID!) { world(id: $id) { activeSceneId } }",
    { id: WORLD_ID },
  );
  return world.activeSceneId;
}

export async function scenes(
  page: Page,
): Promise<{ sceneId: string; name: string }[]> {
  const answer = await data<{ scenes: { sceneId: string; name: string }[] }>(
    page,
    "query ($worldId: UUID!) { scenes(worldId: $worldId) { sceneId name } }",
    { worldId: WORLD_ID },
  );
  return answer.scenes;
}

/** Into play, with the engine up and the Game Master's tools showing. */
export async function enterPlay(page: Page): Promise<void> {
  await page.goto(`/demo/world/${WORLD_ID}/play`);
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("gm-tool-walls")).toBeVisible({
    timeout: 60_000,
  });
}
