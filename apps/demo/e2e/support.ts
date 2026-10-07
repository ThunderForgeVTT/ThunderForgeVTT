import { expect, type Browser, type Page } from "@playwright/test";

/**
 * What the demo's feature specs share: a page whose every request outside
 * the demo's own files is logged, and the backend asked the way the app asks
 * it. `demo.spec.ts` keeps its own copies; it predates this file.
 */

export const WORLD_ID = "d0000000-0000-4000-0002-000000000001";

export interface Opened {
  page: Page;
  /** Every request and socket that is not one of the demo's own files. */
  outside: string[];
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
  const origin = new URL(baseURL as string).origin;
  context.on("request", (request) => {
    const url = new URL(request.url());
    if (url.protocol === "blob:" || url.protocol === "data:") return;
    if (url.origin === origin && url.pathname.startsWith("/demo/")) return;
    outside.push(`${request.method()} ${request.url()}`);
  });
  page.on("websocket", (socket) => outside.push(`SOCKET ${socket.url()}`));
  return { page, outside };
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
