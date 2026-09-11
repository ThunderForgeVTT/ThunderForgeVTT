import { test, expect, type Page } from "@playwright/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  launchSceneByName,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { createScene } from "./fixtures/world-cache";

/**
 * Playtest 2026-09-10 P9: "walls aren't shading properly".
 *
 * They could not. The engine's darkness layer — the sheet over the map, the
 * light pools cut out of it, and the wall shadows cast back into those pools —
 * draws nothing in daylight, and nothing in the product ever told the engine a
 * scene was anything else. `engine-lighting-limits.spec.ts` pinned that as
 * "zero shadow quads at every level". This drives the real control: the Game
 * Master sets the scene's light in the Lights panel, and the shadows appear —
 * for them, for a player who only heard about it on the event channel, and
 * again after a reload.
 */

async function shadowQuads(page: Page): Promise<number> {
  return page.evaluate(async () => {
    const mod = (await import(
      /* @vite-ignore */ "/src/engine/bevy/stats.ts"
    )) as typeof import("../src/engine/bevy/stats");
    const stats = await mod.readEngineStats();
    return stats?.shadowQuads ?? 0;
  });
}

async function wallCount(page: Page): Promise<number> {
  return page.evaluate(() => window.__worldProbe?.state()?.counts.walls ?? 0);
}

type Counts = { tokens: number; walls: number; lights: number; shapes: number };

async function storeCounts(page: Page): Promise<Counts | null> {
  return page.evaluate(
    () => (window.__worldProbe?.state()?.counts ?? null) as Counts | null,
  );
}

type Camera = { x: number; y: number; scale: number };

type EngineProbe = {
  camera?: () => Camera | null;
  hiddenTokens?: () => string[];
};

async function camera(page: Page): Promise<Camera | null> {
  return page.evaluate(
    () =>
      (
        window as unknown as { __engineProbe?: EngineProbe }
      ).__engineProbe?.camera?.() ?? null,
  );
}

async function hiddenTokens(page: Page): Promise<string[]> {
  return page.evaluate(
    () =>
      (
        window as unknown as { __engineProbe?: EngineProbe }
      ).__engineProbe?.hiddenTokens?.() ?? [],
  );
}

async function canvasBox(page: Page) {
  const canvas = page.locator("canvas");
  await canvas.scrollIntoViewIfNeeded();
  const box = await canvas.boundingBox();
  if (!box) {
    throw new Error("Bevy canvas element not found");
  }
  return box;
}

/**
 * Zooms out with the mouse wheel until the camera's scale reaches `minScale`,
 * then waits for the camera to stop gliding.
 *
 * At the default 1:1 camera a point outside two 500-unit light pools is off
 * a 720-pixel-tall canvas, or under the Play chrome at its edges. The wheel
 * is the real control, and the probe says where the camera ended up, so
 * nothing below assumes a zoom level.
 */
async function zoomOutTo(page: Page, minScale: number): Promise<Camera> {
  const box = await canvasBox(page);
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  for (let notch = 0; notch < 30; notch += 1) {
    const cam = await camera(page);
    if (cam && cam.scale >= minScale) break;
    await page.mouse.wheel(0, 120);
    await page.waitForTimeout(150);
  }
  let previous: Camera | null = null;
  await expect
    .poll(
      async () => {
        const cam = await camera(page);
        const settled =
          cam !== null &&
          previous !== null &&
          cam.x === previous.x &&
          cam.y === previous.y &&
          cam.scale === previous.scale;
        previous = cam;
        return settled ? cam.scale : 0;
      },
      {
        timeout: 15_000,
        intervals: [250],
        message: "the camera zooms out with the wheel, and settles",
      },
    )
    .toBeGreaterThanOrEqual(minScale);
  return previous!;
}

/**
 * The mean luminance (Rec. 709, 0..255) of a small square of screen pixels
 * around each world point.
 *
 * Read from a Playwright screenshot, not from the canvas: wgpu does not ask
 * for `preserveDrawingBuffer`, so reading the WebGL canvas back returns
 * transparent black on a timing whim (see `fixtures/canvasPixels.ts`). The
 * PNG is decoded by the page's own decoder, as that fixture does, so there
 * is no decoding dependency. A patch rather than a pixel, so one
 * anti-aliased grid line or wall edge cannot decide the result.
 */
async function luminanceAt(
  page: Page,
  points: { x: number; y: number }[],
): Promise<number[]> {
  const box = await canvasBox(page);
  const viewport = page.viewportSize()!;
  const clip = {
    x: Math.max(box.x, 0),
    y: Math.max(box.y, 0),
    width: Math.min(box.x + box.width, viewport.width) - Math.max(box.x, 0),
    height: Math.min(box.y + box.height, viewport.height) - Math.max(box.y, 0),
  };
  const cam = (await camera(page))!;

  // Only the middle of the canvas is the map: the tool rail, the dock and the
  // dice roller are painted over its edges. A sample point outside this
  // inset would be measuring the chrome, so it fails here rather than there.
  const inset = 0.2;
  const screen = points.map(({ x, y }) => {
    const sx = box.x + box.width / 2 + (x - cam.x) / cam.scale;
    const sy = box.y + box.height / 2 - (y - cam.y) / cam.scale;
    expect(
      sx >= clip.x + clip.width * inset &&
        sx <= clip.x + clip.width * (1 - inset) &&
        sy >= clip.y + clip.height * inset &&
        sy <= clip.y + clip.height * (1 - inset),
      `world (${x}, ${y}) lands at screen (${sx.toFixed(0)}, ${sy.toFixed(0)}), clear of the Play chrome`,
    ).toBe(true);
    return { x: sx - clip.x, y: sy - clip.y };
  });

  const png = await page.screenshot({ clip });
  return page.evaluate(
    async ({ base64, screen, clipWidth }) => {
      const response = await fetch(`data:image/png;base64,${base64}`);
      const bitmap = await createImageBitmap(await response.blob());
      const surface = new OffscreenCanvas(bitmap.width, bitmap.height);
      const context = surface.getContext("2d")!;
      context.drawImage(bitmap, 0, 0);
      // A screenshot is in device pixels; the probe maps to CSS pixels.
      const ratio = bitmap.width / clipWidth;
      const half = 3;
      return screen.map(({ x, y }) => {
        const cx = Math.round(x * ratio);
        const cy = Math.round(y * ratio);
        const { data } = context.getImageData(
          cx - half,
          cy - half,
          half * 2 + 1,
          half * 2 + 1,
        );
        let sum = 0;
        for (let i = 0; i < data.length; i += 4) {
          sum += 0.2126 * data[i] + 0.7152 * data[i + 1] + 0.0722 * data[i + 2];
        }
        return sum / (data.length / 4);
      });
    },
    { base64: png.toString("base64"), screen, clipWidth: clip.width },
  );
}

test.describe("Scene light (playtest 2026-09-10 P9)", () => {
  test("a Game Master dims the scene, its walls cast shadows for everyone, and it stays set", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    await registerAndCreateWorld(page, `E2E Scene Light ${uniqueSuffix()}`);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];
    const sceneId = await createScene(page, worldId, "Lit Scene");
    // Visible to players, so the second client below reads the same scene.
    await graphql(
      page,
      `
        mutation ($sceneId: UUID!) {
          updateSceneHidden(sceneId: $sceneId, hidden: false) {
            sceneId
          }
        }
      `,
      { sceneId },
    );
    await launchSceneByName(page, worldId, "Lit Scene");
    await page.goto(`/world/${worldId}/play`);
    await waitForEngineReady(page);

    // One shadow-casting light, and one vision-blocking wall inside its pool.
    for (const [query, input] of [
      [
        `mutation ($input: GraphQLCreateLightSourceInput!) { createLightSource(input: $input) { lightId } }`,
        { sceneId, x: 0, y: 0, radius: 600, intensity: 1, castsShadows: true },
      ],
      [
        `mutation ($input: GraphQLCreateWallInput!) { createWall(input: $input) { wallId } }`,
        { sceneId, x1: 150, y1: -200, x2: 150, y2: 200, blocksVision: true },
      ],
    ] as const) {
      const result = await graphql<{ errors?: unknown }>(page, query, {
        input,
      });
      expect(result.errors).toBeUndefined();
    }
    await expect.poll(() => wallCount(page), { timeout: 15_000 }).toBe(1);

    // Daylight draws no darkness, so no shadow: where every scene was stuck.
    await page.waitForTimeout(1_000);
    expect(await shadowQuads(page)).toBe(0);

    await page.getByTestId("gm-tool-lights").click();
    const bright = page.getByTestId("scene-ambient-bright");
    const dim = page.getByTestId("scene-ambient-dim");
    const dark = page.getByTestId("scene-ambient-dark");
    await expect(bright).toHaveAttribute("aria-pressed", "true");
    await dim.click();
    await expect(dim).toHaveAttribute("aria-pressed", "true");
    await expect
      .poll(() => shadowQuads(page), {
        timeout: 15_000,
        message: "a dim scene draws the wall's shadow",
      })
      .toBeGreaterThan(0);

    // A player joining now loads the scene as the Game Master left it.
    const player = await inviteAndJoinAsPlayer(browser, page, worldId);
    await player.goto(`/world/${worldId}/play`);
    await waitForEngineReady(player);
    await expect.poll(() => wallCount(player), { timeout: 15_000 }).toBe(1);
    await expect
      .poll(() => shadowQuads(player), {
        timeout: 30_000,
        message: "the player's canvas loads the scene dim",
      })
      .toBeGreaterThan(0);

    // And follows a change live, without reloading — the event channel.
    await bright.click();
    await expect
      .poll(() => shadowQuads(player), {
        timeout: 15_000,
        message: "back to daylight, the player's shadows go with it",
      })
      .toBe(0);
    await dark.click();
    await expect
      .poll(() => shadowQuads(player), { timeout: 15_000 })
      .toBeGreaterThan(0);

    // It is the scene's setting, not this session's: a reload comes back dark.
    await page.reload();
    await waitForEngineReady(page);
    await expect
      .poll(() => shadowQuads(page), { timeout: 30_000 })
      .toBeGreaterThan(0);
    await page.getByTestId("gm-tool-lights").click();
    await expect(page.getByTestId("scene-ambient-dark")).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    await player.context().close();
  });

  test("each light is stopped only by its own walls", async ({ page }) => {
    test.setTimeout(300_000);

    await registerAndCreateWorld(page, `E2E Two Lights ${uniqueSuffix()}`);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];
    const sceneId = await createScene(page, worldId, "Two Lights");
    await launchSceneByName(page, worldId, "Two Lights");
    await page.goto(`/world/${worldId}/play`);
    await waitForEngineReady(page);

    const dark = await graphql<{ errors?: unknown }>(
      page,
      `
        mutation ($sceneId: UUID!) {
          updateSceneAmbientLight(sceneId: $sceneId, ambientLight: "dark") {
            sceneId
          }
        }
      `,
      { sceneId },
    );
    expect(dark.errors).toBeUndefined();

    // Two lights either side of one wall. Radius 500 rather than 450 so that
    // P below sits inside A's reach (450 < 500) as well as B's bright core:
    // at exactly A's radius, A's shadow would have nothing to darken and the
    // test could not tell the old overlay from the new per-light stop.
    //
    // The bright surface is a filled rectangle shape, not an imported map.
    // Shapes draw under the darkness, a rect is one flat sprite of a known
    // colour, and nothing about it depends on a fixture image's content —
    // a map would put its own dark walls and floor texture under the sample
    // points. The empty canvas's clear colour is already dark, so without
    // it lit and unlit would differ by a few levels at best.
    for (const [query, input] of [
      [
        `mutation ($input: GraphQLCreateShapeInput!) { createShape(input: $input) { shapeId } }`,
        {
          sceneId,
          kind: "RECT",
          geometry: { x: -1200, y: -800, w: 2400, h: 1600 },
        },
      ],
      [
        `mutation ($input: GraphQLCreateLightSourceInput!) { createLightSource(input: $input) { lightId } }`,
        {
          sceneId,
          x: -300,
          y: 0,
          radius: 500,
          intensity: 1,
          castsShadows: true,
        },
      ],
      [
        `mutation ($input: GraphQLCreateLightSourceInput!) { createLightSource(input: $input) { lightId } }`,
        {
          sceneId,
          x: 300,
          y: 0,
          radius: 500,
          intensity: 1,
          castsShadows: true,
        },
      ],
      [
        `mutation ($input: GraphQLCreateWallInput!) { createWall(input: $input) { wallId } }`,
        { sceneId, x1: 0, y1: -150, x2: 0, y2: 150, blocksVision: true },
      ],
    ] as const) {
      const result = await graphql<{ errors?: unknown }>(page, query, {
        input,
      });
      expect(result.errors).toBeUndefined();
    }
    await expect
      .poll(() => storeCounts(page), { timeout: 15_000 })
      .toMatchObject({ walls: 1, lights: 2, shapes: 1 });
    await expect
      .poll(() => shadowQuads(page), {
        timeout: 15_000,
        message: "a dark scene draws the wall's shadows",
      })
      .toBeGreaterThan(0);

    await zoomOutTo(page, 2.5);

    // P: lit by B, and behind the wall from A.
    // Q: its mirror, lit by A, behind the wall from B.
    // R: outside both pools (541 units from each light), still on the rect.
    const P = { x: 150, y: 0 };
    const Q = { x: -150, y: 0 };
    const R = { x: 0, y: 450 };

    // Retried, because the darkness is rebuilt a frame or two after the
    // store changes and a screenshot can land in between.
    await expect(async () => {
      const [p, q, r] = await luminanceAt(page, [P, Q, R]);
      const reading = `P=${p.toFixed(1)} Q=${q.toFixed(1)} R=${r.toFixed(1)}`;

      // A dark scene takes 92% off an unlit fragment, and P and Q are each
      // inside a light's full-bright core (150 < radius / 2), so the gap
      // between lit and unlit rect is well over 100 levels. 60 leaves room
      // for a tinted pool and anti-aliasing, and is still far more than a
      // shadowed P could show: under a shadow it is as dark as R.
      //
      // Before the fix, that is what P was. Shadows were one overlay for the
      // whole layer, so A's shadow off the wall was painted across B's pool
      // and P came out as dark as the unlit floor.
      expect(q, `Q is lit by A (${reading})`).toBeGreaterThan(r + 60);
      expect(
        p,
        `P is lit by B, whatever A's wall does (${reading})`,
      ).toBeGreaterThan(r + 60);
      // Mirror images across the wall, both in a full-bright core: they
      // should match. 0.8 allows for the wall and light markers the Game
      // Master sees drawn nearby, and for rounding.
      expect(
        Math.min(p, q) / Math.max(p, q),
        `P and Q are lit alike (${reading})`,
      ).toBeGreaterThan(0.8);
    }).toPass({ timeout: 20_000 });
  });

  test("a player sees the board through their own token; the Game Master sees every token", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    await registerAndCreateWorld(page, `E2E Line of Sight ${uniqueSuffix()}`);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];
    const sceneId = await createScene(page, worldId, "Sight Scene");
    await graphql(
      page,
      `
        mutation ($sceneId: UUID!) {
          updateSceneHidden(sceneId: $sceneId, hidden: false) {
            sceneId
          }
        }
      `,
      { sceneId },
    );
    await launchSceneByName(page, worldId, "Sight Scene");
    await page.goto(`/world/${worldId}/play`);
    await waitForEngineReady(page);

    // The player joins first: their token has to be owned by an account
    // that exists, and is a member, before it can be made theirs.
    const player = await inviteAndJoinAsPlayer(browser, page, worldId);
    const me = await graphql<{ data?: { me?: { id: string } } }>(
      player,
      `
        query {
          me {
            id
          }
        }
      `,
      {},
    );
    const playerUserId = me.data!.me!.id;

    const createToken = `
      mutation ($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) {
          tokenId
        }
      }
    `;
    const own = await graphql<{
      data?: { createToken?: { tokenId: string } };
    }>(page, createToken, { input: { sceneId, x: -200, y: 0 } });
    const ownTokenId = own.data!.createToken!.tokenId;
    const npc = await graphql<{
      data?: { createToken?: { tokenId: string } };
    }>(page, createToken, {
      input: { sceneId, x: 200, y: 0, tokenType: "npc" },
    });
    const npcTokenId = npc.data!.createToken!.tokenId;

    // Owned and primary: the token the web names to the engine as this
    // player's eyes. Created without either because `createToken` takes
    // neither; `updateToken` is where they are set.
    const owned = await graphql<{ errors?: unknown }>(
      page,
      `
        mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
          updateToken(tokenId: $tokenId, input: $input) {
            tokenId
          }
        }
      `,
      {
        tokenId: ownTokenId,
        input: { ownerUserId: playerUserId, isPrimary: true },
      },
    );
    expect(owned.errors).toBeUndefined();

    const wall = await graphql<{
      data?: { createWall?: { wallId: string } };
      errors?: unknown;
    }>(
      page,
      `
        mutation ($input: GraphQLCreateWallInput!) {
          createWall(input: $input) {
            wallId
          }
        }
      `,
      {
        input: { sceneId, x1: 0, y1: -300, x2: 0, y2: 300, blocksVision: true },
      },
    );
    expect(wall.errors).toBeUndefined();
    const wallId = wall.data!.createWall!.wallId;

    await player.goto(`/world/${worldId}/play`);
    await waitForEngineReady(player);
    await expect
      .poll(() => storeCounts(player), { timeout: 15_000 })
      .toMatchObject({ tokens: 2, walls: 1 });

    // A bright scene: nothing here is about the dark. The wall alone keeps
    // the NPC from the player's token, so the player's canvas hides it.
    await expect
      .poll(() => hiddenTokens(player), {
        timeout: 15_000,
        message: "the wall hides the NPC from the player's token",
      })
      .toContain(npcTokenId);
    expect(
      await hiddenTokens(player),
      "a player always sees their own token",
    ).not.toContain(ownTokenId);

    // The Game Master sees through no token, so a wall hides nothing from
    // them. Given time for a hide that was going to happen to have happened.
    await expect
      .poll(() => storeCounts(page), { timeout: 15_000 })
      .toMatchObject({ tokens: 2, walls: 1 });
    await page.waitForTimeout(1_500);
    expect(
      await hiddenTokens(page),
      "the Game Master's canvas hides no token behind a wall",
    ).not.toContain(npcTokenId);

    // Take the wall away, and the player's token can see the NPC again —
    // live, over the event channel, without a reload.
    const deleted = await graphql<{ errors?: unknown }>(
      page,
      `
        mutation ($wallId: UUID!) {
          deleteWall(wallId: $wallId)
        }
      `,
      { wallId },
    );
    expect(deleted.errors).toBeUndefined();
    await expect.poll(() => wallCount(player), { timeout: 15_000 }).toBe(0);
    await expect
      .poll(() => hiddenTokens(player), {
        timeout: 15_000,
        message: "with the wall gone, the player sees the NPC",
      })
      .not.toContain(npcTokenId);

    await player.context().close();
  });
});
