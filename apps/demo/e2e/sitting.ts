import type { Browser, Page } from "@playwright/test";
import { ask as askPage, openDemo, type Opened } from "./support";

/**
 * What the spec 074 sitting specs (`demo-board`, `demo-pages`,
 * `demo-limits`) share: one page a whole file runs on, and helpers that
 * close over it, so a test reads `currentScene()` rather than
 * `currentScene(page)`. They were `demo.spec.ts`'s own, before it was split.
 */

export const WORLD_ID = "d0000000-0000-4000-0002-000000000001";

export type Actor = {
  id: string;
  label: string;
  isNpc: boolean;
  ownedBy: string;
  visibleToPlayers: boolean;
  myPermissionLevel: string;
};

export type Scene = {
  sceneId: string;
  name: string;
  walls: {
    wallId: string;
    doorState: string;
    x1: number;
    y1: number;
    x2: number;
    y2: number;
  }[];
  lightSources: { lightId: string }[];
  tokens: { tokenId: string; x: number; y: number; photoUrl: string | null }[];
};

/** Where the fighter stands when the demo ships: on the road, mid-map. */
export const FIGHTER_START = { x: -297.5, y: 85 };

/**
 * `openDemo`, with the page's `fetch` watched for actor pictures: which were
 * asked of it, and by whom it was called. Whatever the guard installs as
 * `window.fetch` is wrapped as it is read, so a call from the engine is seen
 * as well as one from a script.
 */
export async function openSitting(
  browser: Browser,
  baseURL: string | undefined,
): Promise<Opened> {
  const opened = await openDemo(browser, baseURL);
  await opened.page.addInitScript(() => {
    const seen: string[] = [];
    let current = window.fetch;
    Object.assign(window, { __actorArtFetched: seen });
    Object.defineProperty(window, "fetch", {
      configurable: true,
      get() {
        const target = current;
        return (input: RequestInfo | URL, init?: RequestInit) => {
          const url = input instanceof Request ? input.url : String(input);
          if (url.includes("/api/actor-assets/")) seen.push(url);
          return target.call(window, input, init);
        };
      },
      set(value: typeof fetch) {
        current = value;
      },
    });
  });
  return opened;
}

/** The helpers, bound to the page `pageOf` returns when each is called. */
export function sitting(pageOf: () => Page) {
  /**
   * Ask the demo's own backend, the way the app does. The page's `fetch` is
   * the demo's, so this goes nowhere; if it ever did, the request log would
   * say so.
   */
  async function ask<T>(query: string, variables: object = {}) {
    return askPage<T>(pageOf(), query, variables) as Promise<{
      status: number;
      body: {
        data?: T;
        errors?: { message?: string; extensions?: { code?: string } }[];
      };
    }>;
  }

  /** What this page load has refused as "not part of the demo", by name. */
  async function refusedSoFar(): Promise<string[]> {
    return pageOf().evaluate(() => [
      ...((window as { __thunderforgeDemoRefused?: string[] })
        .__thunderforgeDemoRefused ?? []),
    ]);
  }

  async function scenes(): Promise<{ sceneId: string; name: string }[]> {
    const answer = await ask<{ scenes: { sceneId: string; name: string }[] }>(
      "query ($worldId: UUID!) { scenes(worldId: $worldId) { sceneId name } }",
      { worldId: WORLD_ID },
    );
    if (!answer.body.data) throw new Error(JSON.stringify(answer.body.errors));
    return answer.body.data.scenes;
  }

  /** The scene the play field is showing, as the demo's backend holds it now. */
  async function currentScene(): Promise<Scene> {
    const world = await ask<{ world: { activeSceneId: string } }>(
      "query ($id: UUID!) { world(id: $id) { activeSceneId } }",
      { id: WORLD_ID },
    );
    const active = world.body.data?.world.activeSceneId;
    const scene = (await scenes()).find((s) => s.sceneId === active);
    if (!scene) throw new Error("no scene is active");
    const answer = await ask<Omit<Scene, "sceneId" | "name">>(
      `query ($sceneId: UUID!) {
        walls(sceneId: $sceneId) { wallId doorState x1 y1 x2 y2 }
        lightSources(sceneId: $sceneId) { lightId }
        tokens(sceneId: $sceneId) { tokenId x y photoUrl }
      }`,
      { sceneId: scene.sceneId },
    );
    if (!answer.body.data) throw new Error(JSON.stringify(answer.body.errors));
    return { ...scene, ...answer.body.data };
  }

  async function cast(): Promise<Actor[]> {
    const answer = await ask<{ worldActors: Actor[] }>(
      `query ($worldId: UUID!) { worldActors(worldId: $worldId) {
        id label isNpc ownedBy visibleToPlayers myPermissionLevel
      } }`,
      { worldId: WORLD_ID },
    );
    if (!answer.body.data) throw new Error(JSON.stringify(answer.body.errors));
    return answer.body.data.worldActors;
  }

  /**
   * Board coordinates to the screen, at the camera a scene opens with:
   * centred on the origin, one board unit to a pixel, y up. The built bundle
   * carries no engine probe to ask, and the tests never move the camera.
   */
  async function toScreen(point: { x: number; y: number }) {
    const box = await pageOf().locator("canvas").boundingBox();
    if (!box) throw new Error("the board's canvas is not on the page");
    return {
      x: box.x + box.width / 2 + point.x,
      y: box.y + box.height / 2 - point.y,
    };
  }

  async function clickBoard(
    point: { x: number; y: number },
    button: "left" | "right" = "left",
  ) {
    const page = pageOf();
    const at = await toScreen(point);
    await page.mouse.move(at.x, at.y);
    await page.mouse.down({ button });
    // The engine reads the pointer once a frame; a press and release inside
    // one frame is a click it never saw.
    await page.waitForTimeout(120);
    await page.mouse.up({ button });
  }

  /** A press, a move, a release between two board points. */
  async function dragBoard(
    from: { x: number; y: number },
    to: { x: number; y: number },
  ) {
    const page = pageOf();
    const a = await toScreen(from);
    const b = await toScreen(to);
    await page.mouse.move(a.x, a.y);
    await page.mouse.down();
    await page.waitForTimeout(120);
    await page.mouse.move(b.x, b.y, { steps: 12 });
    await page.waitForTimeout(120);
    await page.mouse.up();
  }

  /** Spec 076: what the engine shows of the map at a board point. */
  async function sightAt(point: { x: number; y: number }) {
    return pageOf().evaluate(
      ([x, y]) =>
        (
          window as unknown as {
            __engineProbe?: {
              sight?: (
                x: number,
                y: number,
              ) => { looking: boolean; seen: boolean };
            };
          }
        ).__engineProbe?.sight?.(x, y) ?? { looking: false, seen: true },
      [point.x, point.y] as const,
    );
  }

  /** The icon the board draws on a door, from the engine probe; `null` for none. */
  async function doorIconOf(wallId: string): Promise<string | null> {
    return pageOf().evaluate(
      (id) =>
        (
          window as unknown as {
            __engineProbe?: {
              doorIcons?: () => { wallId: string; icon: string }[];
            };
          }
        ).__engineProbe
          ?.doorIcons?.()
          .find((row) => row.wallId === id)?.icon ?? null,
      wallId,
    );
  }

  /**
   * A board point on the screen wherever the camera is now: a player's board
   * opens on their own token, not on the origin `toScreen` assumes.
   */
  async function toScreenNow(point: { x: number; y: number }) {
    const page = pageOf();
    const box = await page.locator("canvas").boundingBox();
    if (!box) throw new Error("the board's canvas is not on the page");
    const cam = (await page.evaluate(
      () =>
        (
          window as unknown as {
            __engineProbe?: {
              camera?: () => { x: number; y: number; scale: number } | null;
            };
          }
        ).__engineProbe?.camera?.() ?? null,
    )) ?? { x: 0, y: 0, scale: 1 };
    const scale = cam.scale > 0 ? cam.scale : 1;
    return {
      x: box.x + box.width / 2 + (point.x - cam.x) / scale,
      y: box.y + box.height / 2 - (point.y - cam.y) / scale,
    };
  }

  async function pressDoorIcon(point: { x: number; y: number }) {
    const page = pageOf();
    const at = await toScreenNow(point);
    await page.mouse.move(at.x, at.y);
    await page.mouse.down();
    await page.waitForTimeout(120);
    await page.mouse.up();
  }

  async function doorLocked(wallId: string) {
    const { sceneId } = await currentScene();
    const answer = await ask<{ walls: { wallId: string; locked: boolean }[] }>(
      "query ($sceneId: UUID!) { walls(sceneId: $sceneId) { wallId locked } }",
      { sceneId },
    );
    return answer.body.data?.walls.find((w) => w.wallId === wallId)?.locked;
  }

  return {
    ask,
    refusedSoFar,
    scenes,
    currentScene,
    cast,
    toScreen,
    clickBoard,
    dragBoard,
    sightAt,
    doorIconOf,
    toScreenNow,
    pressDoorIcon,
    doorLocked,
  };
}
