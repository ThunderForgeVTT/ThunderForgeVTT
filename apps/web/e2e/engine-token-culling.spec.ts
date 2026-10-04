import { expect, test, type Page } from "./fixtures/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { camera, canvasBox, type Camera } from "./fixtures/lightingProbe";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Viewport culling of token furniture — what it saves, and that nothing goes
 * missing.
 *
 * A token outside the camera's view, padded by a quarter of the view's size on
 * every side, carries no name and no bars (`plugins/token_culling.rs`). This
 * asks a real engine two questions about that:
 *
 * 1. **Is anything a viewer could see ever missing?** At rest, and after every
 *    pan, every token inside the padded view has its furniture. This is the
 *    question that matters: a saving that drops a bar from a token on screen
 *    is a bug with a frame rate attached.
 * 2. **What does it save?** The same board, in the same session, sampled with
 *    culling on and with it off.
 *
 * # Why a sibling of `engine-status-limits.spec.ts`
 *
 * That file measured what status displays cost at 3,200 tokens: 16,003 sprites
 * against 3,203, and a third of the frame rate. This measures how much of that
 * is given back when the camera — as it always does on a board that size —
 * shows a small part of it. The methodology is copied from there rather than
 * invented: tokens through the real mutation, created from a page that is not
 * running the engine, a fresh load, the same settle window, the same sampling
 * window, the median and not the mean.
 *
 * # Why both sides come from one page
 *
 * `set_token_culling` switches the feature inside a running engine, so "on"
 * and "off" are the same tokens, the same camera, the same build and the same
 * warmed caches a few seconds apart. Two page loads would measure the
 * difference between two page loads.
 *
 * # What is read back
 *
 * `token_footprints()` reports, per token, whether a nameplate and bars are
 * actually parented to it (`nameY`, `barWidth`) — entities that exist, not a
 * flag saying they should. `engine_stats()` gives the sprite count and the
 * engine's own frame time. The camera probe gives the view. Nothing is
 * inferred from pixels.
 */

/**
 * How many tokens are on the board. 3,200 is the measured lane's anchor
 * (`engine-limits.spec.ts`, `engine-status-limits.spec.ts`), so a figure from
 * here sits beside figures already written down.
 *
 * `TOKEN_CULLING_TOKENS` overrides it, for smoke-running the harness at a size
 * that finishes in a minute. A figure quoted anywhere is from the default.
 */
const TOKENS = Number(process.env.TOKEN_CULLING_TOKENS ?? "3200");

/** World units between tokens — the spacing the sibling specs lay out at. */
const SPACING = 140;

/** Seconds of sampling per condition, after the scene has settled. */
const SAMPLE_MS = 4_000;

/** Settle window before sampling — loaders and first spawns are not steady state. */
const SETTLE_MS = 4_000;

/** How long to let furniture finish arriving, or leaving, before giving up. */
const FURNITURE_SETTLE_MS = 120_000;

/** The padding the feature promises, as a fraction of the view on each side. */
const VIEW_PAD = 0.25;

/**
 * How far outside the view a token must be before this file insists it is
 * culled. The engine lets go at 40% of the view plus the token's own reach
 * (half a token and 192 units of furniture); a whole view beyond the edge is
 * past that at any size this runs at, with room to spare.
 */
const SURELY_OUTSIDE = 1.0;

interface Footprint {
  tokenId: string;
  x: number;
  y: number;
  nameY: number | null;
  barWidth: number | null;
}

interface Sample {
  culling: boolean;
  tokens: number;
  fps: number;
  frameTimeMs: number;
  sprites: number;
  /** Tokens the engine itself reports as culled (`engine_stats`). */
  culled: number;
  /** Tokens with bars actually built. */
  withBars: number;
  /** Tokens with a nameplate actually built. */
  withNames: number;
  samples: number;
}

async function gql<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  const res = await graphql<{ data?: T; errors?: { message: string }[] }>(
    page,
    query,
    variables,
  );
  if (res.errors?.length || !res.data) {
    throw new Error(`GraphQL failed: ${JSON.stringify(res.errors ?? res)}`);
  }
  return res.data;
}

/** The engine's own counters, or null — see `engine-status-limits.spec.ts`. */
async function readStats(page: Page) {
  try {
    return await page.evaluate(async () => {
      const mod = (await import(
        /* @vite-ignore */ "/src/engine/bevy/stats.ts"
      )) as typeof import("../src/engine/bevy/stats");
      return mod.readEngineStats();
    });
  } catch {
    return null;
  }
}

/** Every token as the engine draws it. Mirrored a few times a second. */
async function footprints(page: Page): Promise<Footprint[]> {
  try {
    return await page.evaluate(() =>
      (
        (
          window as unknown as {
            __engineProbe?: { tokenFootprints?: () => unknown[] };
          }
        ).__engineProbe?.tokenFootprints?.() ?? []
      ).map((row) => row as Footprint),
    );
  } catch {
    return [];
  }
}

/**
 * Send the engine a command the way the application does: through the bound
 * world store, whose bridge forwards every command to `apply_world_command`.
 * Neither command used here changes the store's own state.
 */
async function engineCommand(
  page: Page,
  command: Record<string, unknown>,
): Promise<void> {
  const sent = await page.evaluate(async (cmd) => {
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    const store = bevy.getBoundWorldStore();
    if (!store) return false;
    store.dispatch(cmd as never);
    return true;
  }, command);
  expect(sent, "the engine's world store must be bound").toBe(true);
}

/** Move the camera and wait until it has arrived and stopped. */
async function panTo(page: Page, x: number, y: number): Promise<Camera> {
  await engineCommand(page, { type: "set_camera", x, y });
  let previous: Camera | null = null;
  let arrived: Camera | null = null;
  await expect
    .poll(
      async () => {
        const cam = await camera(page);
        const settled =
          cam !== null &&
          previous !== null &&
          cam.x === previous.x &&
          cam.y === previous.y &&
          cam.scale === previous.scale &&
          Math.abs(cam.x - x) < 1 &&
          Math.abs(cam.y - y) < 1;
        previous = cam;
        if (settled) arrived = cam;
        return settled;
      },
      {
        timeout: 20_000,
        intervals: [250],
        message: `the camera reaches (${x}, ${y}) and settles`,
      },
    )
    .toBe(true);
  return arrived as unknown as Camera;
}

/**
 * The half-extent of what the camera shows, in world units. One world unit is
 * one CSS pixel at scale 1, and scale is world units per pixel.
 */
async function halfView(page: Page, cam: Camera) {
  const box = await canvasBox(page);
  return { x: (box.width * cam.scale) / 2, y: (box.height * cam.scale) / 2 };
}

/**
 * Wait until furniture has stopped arriving or leaving: the sprite count holds
 * for five seconds. A plateau rather than a target, for the reason the sibling
 * spec gives — how many sprites a token draws is the feature's business.
 */
async function waitForPlateau(page: Page): Promise<void> {
  let sprites = -1;
  let steady = 0;
  const deadline = Date.now() + FURNITURE_SETTLE_MS;
  while (Date.now() < deadline) {
    const stats = await readStats(page);
    const count = stats?.sprites ?? -1;
    steady = count === sprites && count >= 0 ? steady + 1 : 0;
    sprites = count;
    if (steady >= 10) return;
    await page.waitForTimeout(500);
  }
}

/** Median frame time over the window — one stall must not write the figure. */
async function sampleSteadyState(
  page: Page,
  culling: boolean,
): Promise<Sample> {
  const readings: { fps: number; frameTimeMs: number }[] = [];
  let last = await readStats(page);
  const deadline = Date.now() + SAMPLE_MS;
  while (Date.now() < deadline) {
    const stats = await readStats(page);
    if (stats && stats.fps > 0) {
      readings.push({ fps: stats.fps, frameTimeMs: stats.frameTimeMs });
      last = stats;
    }
    await page.waitForTimeout(200);
  }
  readings.sort((a, b) => a.frameTimeMs - b.frameTimeMs);
  const mid = readings[Math.floor(readings.length / 2)] ?? {
    fps: 0,
    frameTimeMs: 0,
  };
  const rows = await footprints(page);
  return {
    culling,
    tokens: last?.tokens ?? 0,
    fps: Math.round(mid.fps),
    frameTimeMs: Number(mid.frameTimeMs.toFixed(2)),
    sprites: last?.sprites ?? 0,
    culled: last?.tokensCulled ?? 0,
    withBars: rows.filter((row) => row.barWidth !== null).length,
    withNames: rows.filter((row) => row.nameY !== null).length,
    samples: readings.length,
  };
}

/** An NPC with resources, so every token placed from it draws bars. */
async function displayableActor(
  page: Page,
  worldId: string,
  suffix: string,
): Promise<string> {
  await gql(
    page,
    `mutation ($input: UpdateWorldGameSystemInput!) {
      updateWorldGameSystem(input: $input) { id }
    }`,
    { input: { worldId, gameSystemId: "genie" } },
  );
  const actor = await gql<{ createActor: { id: string } }>(
    page,
    `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
    {
      input: {
        worldId,
        label: `Mob ${suffix}`,
        isNpc: true,
        gameSystemId: "genie",
      },
    },
  );
  await gql(
    page,
    `mutation ($input: GraphQLUpdateActorSystemDataInput!) {
      updateActorSystemData(input: $input) { id }
    }`,
    {
      input: {
        actorId: actor.createActor.id,
        gameSystemId: "genie",
        dataType: "resource_data",
        data: {
          current_health: 41,
          max_health: 60,
          current_wish_points: 2,
          max_wish_points: 5,
        },
      },
    },
  );
  return actor.createActor.id;
}

/**
 * Create `count` tokens in a square grid centred on the origin, through the
 * real mutation and from a page that is not running the engine — a thousand
 * live arrivals into a running canvas make the application reload itself.
 */
async function addTokens(
  page: Page,
  worldId: string,
  sceneId: string,
  count: number,
  actorId: string,
): Promise<void> {
  await page.goto(`/world/${worldId}/staging`);
  const side = Math.ceil(Math.sqrt(count));
  // In chunks, so no single evaluate is long enough for a reload to land in
  // the middle of it and take the whole board with it.
  const CHUNK = 200;
  for (let done = 0; done < count; done += CHUNK) {
    await page.evaluate(
      async ({ scene, howMany, offset, gridSide, actor, spacing }) => {
        const csrf = document.cookie
          .split(";")
          .map((part) => part.trim())
          .find((part) => part.startsWith("csrf_token="))
          ?.slice("csrf_token=".length);
        const create = async (i: number) => {
          const n = i + offset;
          const x = ((n % gridSide) - gridSide / 2) * spacing;
          const y = (Math.floor(n / gridSide) - gridSide / 2) * spacing;
          const res = await fetch("/api/graphql", {
            method: "POST",
            credentials: "same-origin",
            headers: {
              "Content-Type": "application/json",
              ...(csrf ? { "x-csrf-token": csrf } : {}),
            },
            body: JSON.stringify({
              query: `mutation ($input: GraphQLCreateTokenInput!) {
                createToken(input: $input) { tokenId }
              }`,
              variables: {
                input: {
                  sceneId: scene,
                  x,
                  y,
                  actorId: actor,
                  tokenType: "npc",
                },
              },
            }),
          });
          const body = await res.json();
          if (body.errors) {
            throw new Error(
              `createToken failed: ${JSON.stringify(body.errors)}`,
            );
          }
        };
        const IN_FLIGHT = 12;
        for (let start = 0; start < howMany; start += IN_FLIGHT) {
          await Promise.all(
            Array.from(
              { length: Math.min(IN_FLIGHT, howMany - start) },
              (_, k) => create(start + k),
            ),
          );
        }
      },
      {
        scene: sceneId,
        howMany: Math.min(CHUNK, count - done),
        offset: done,
        gridSide: side,
        actor: actorId,
        spacing: SPACING,
      },
    );
  }
}

/**
 * What is wrong with the board as the camera now sees it, if anything.
 *
 * Polled rather than read once: the footprint mirror refreshes every ten
 * frames, so the first reading after a pan can predate it.
 */
async function culledCorrectly(
  page: Page,
  cam: Camera,
): Promise<{
  missingInView: string[];
  missingInPadding: string[];
  keptFarAway: string[];
  inView: number;
  furnished: number;
}> {
  const half = await halfView(page, cam);
  const rows = await footprints(page);
  const within = (row: Footprint, factor: number) =>
    Math.abs(row.x - cam.x) <= half.x * factor &&
    Math.abs(row.y - cam.y) <= half.y * factor;
  const furnished = (row: Footprint) => row.barWidth !== null;
  return {
    // On screen. The claim nobody would forgive being false.
    missingInView: rows
      .filter((row) => within(row, 1) && !furnished(row))
      .map((row) => row.tokenId),
    // Off screen but inside the promised 25%: what a fast pan lands on.
    missingInPadding: rows
      .filter((row) => within(row, 1 + 2 * VIEW_PAD) && !furnished(row))
      .map((row) => row.tokenId),
    // A whole view beyond the edge and still carrying furniture: not culled.
    keptFarAway: rows
      .filter((row) => !within(row, 1 + 2 * SURELY_OUTSIDE) && furnished(row))
      .map((row) => row.tokenId),
    inView: rows.filter((row) => within(row, 1)).length,
    furnished: rows.filter(furnished).length,
  };
}

async function expectCulledCorrectly(page: Page, cam: Camera, where: string) {
  await expect
    .poll(
      async () => {
        const state = await culledCorrectly(page, cam);
        return {
          missingInView: state.missingInView.length,
          missingInPadding: state.missingInPadding.length,
          keptFarAway: state.keptFarAway.length,
        };
      },
      {
        timeout: 30_000,
        intervals: [500],
        message:
          `${where}: every token in the padded view has its bars, and no ` +
          `token a whole view away still carries any`,
      },
    )
    .toEqual({ missingInView: 0, missingInPadding: 0, keptFarAway: 0 });
  return culledCorrectly(page, cam);
}

test("token culling: off-screen tokens carry nothing, and panning never finds one missing", async ({
  page,
}) => {
  // A measurement run, not a unit test. Creating the tokens through the real
  // mutation path is most of the wall clock.
  test.setTimeout(45 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Token Culling ${suffix}`);
  const [sceneId] = await sceneIds(page, worldId);
  const actorId = await displayableActor(page, worldId, suffix);
  await addTokens(page, worldId, sceneId, TOKENS, actorId);

  // Load, settle, sample both ways — and be willing to do it again. The
  // application reloads its own page when the live connection drops, which
  // lands in a sampling window as a run of missed readings; a window that
  // lost its page is retried rather than reported.
  let on: Sample | null = null;
  let off: Sample | null = null;
  let atRest: Awaited<ReturnType<typeof culledCorrectly>> | null = null;
  const pans: { x: number; y: number; inView: number; furnished: number }[] =
    [];
  for (let attempt = 1; attempt <= 3 && !(on && off); attempt += 1) {
    on = null;
    off = null;
    pans.length = 0;
    await page.goto(`/world/${worldId}/play`);
    await waitForEngineReady(page);
    await page.waitForTimeout(SETTLE_MS);

    // The middle of the board, so the view is surrounded by tokens on every
    // side and "most of them are off-screen" is true in every direction.
    const centre = await panTo(page, 0, 0);
    await expect
      .poll(async () => (await readStats(page))?.tokens ?? 0, {
        timeout: FURNITURE_SETTLE_MS,
        intervals: [1_000],
        message: "every token reaches the engine",
      })
      .toBeGreaterThanOrEqual(TOKENS);
    await waitForPlateau(page);

    // 1. At rest, culling on (the default).
    atRest = await expectCulledCorrectly(page, centre, "at rest");
    const sampledOn = await sampleSteadyState(page, true);

    // 2. Panning. Each stop is far enough from the last that every token in
    // view was culled a moment ago — a diagonal walk to one corner of the
    // board, then straight across it, then home.
    const reach = (Math.ceil(Math.sqrt(TOKENS)) * SPACING) / 2;
    const stops: [number, number][] = [
      [reach * 0.45, reach * 0.45],
      [reach * 0.9, reach * 0.9],
      [-reach * 0.9, reach * 0.9],
      [-reach * 0.9, -reach * 0.9],
      [0, 0],
    ];
    for (const [x, y] of stops) {
      const cam = await panTo(page, x, y);
      const state = await expectCulledCorrectly(
        page,
        cam,
        `after panning to (${Math.round(x)}, ${Math.round(y)})`,
      );
      pans.push({
        x: Math.round(x),
        y: Math.round(y),
        inView: state.inView,
        furnished: state.furnished,
      });
    }

    // 3. The same board, same camera, culling off.
    await engineCommand(page, { type: "set_token_culling", enabled: false });
    await expect
      .poll(
        async () =>
          (await footprints(page)).filter((row) => row.barWidth !== null)
            .length,
        {
          timeout: FURNITURE_SETTLE_MS,
          intervals: [1_000],
          message: "with culling off every token draws its bars again",
        },
      )
      .toBeGreaterThanOrEqual(TOKENS);
    await waitForPlateau(page);
    const sampledOff = await sampleSteadyState(page, false);

    // 4. And back on: the furniture leaves again, without a reload.
    await engineCommand(page, { type: "set_token_culling", enabled: true });
    await expectCulledCorrectly(page, centre, "culling switched back on");

    if (sampledOn.samples > 5 && sampledOff.samples > 5) {
      on = sampledOn;
      off = sampledOff;
    } else {
      console.log(
        `[token-culling] attempt=${attempt} discarded: only ` +
          `${sampledOn.samples}/${sampledOff.samples} readings`,
      );
    }
  }
  if (!on || !off || !atRest) {
    throw new Error("the page never held still long enough to sample");
  }

  // This run's product is the line it prints; the assertions say less.
  for (const sample of [on, off]) {
    console.log(
      `[token-culling] culling=${sample.culling ? "on " : "off"} ` +
        `tokens=${String(sample.tokens).padStart(4)} ` +
        `fps=${String(sample.fps).padStart(3)} ` +
        `frame=${String(sample.frameTimeMs).padStart(7)}ms ` +
        `sprites=${String(sample.sprites).padStart(5)} ` +
        `culled=${String(sample.culled).padStart(4)} ` +
        `withBars=${String(sample.withBars).padStart(4)} ` +
        `withNames=${String(sample.withNames).padStart(4)} ` +
        `samples=${sample.samples}`,
    );
  }
  console.log(
    `[token-culling] result=${JSON.stringify({
      tokens: TOKENS,
      spacing: SPACING,
      viewPad: VIEW_PAD,
      inViewAtRest: atRest.inView,
      on,
      off,
      spritesSaved: off.sprites - on.sprites,
      frameTimeSavedMs: Number((off.frameTimeMs - on.frameTimeMs).toFixed(2)),
      pans,
    })}`,
  );

  // The engine must have reported at all: a zero is "never ran", not "slow".
  expect(on.fps, "a real frame-rate reading with culling on").toBeGreaterThan(
    0,
  );
  expect(off.fps, "a real frame-rate reading with culling off").toBeGreaterThan(
    0,
  );

  // Culling never removes a token. Everything that counts tokens counts the
  // same board either way.
  expect(on.tokens, "every token is on the board").toBeGreaterThanOrEqual(
    TOKENS,
  );
  expect(on.tokens, "culled tokens still exist").toBe(off.tokens);

  // The comparison is only a comparison if the off side really drew it all.
  expect(
    off.withBars,
    "with culling off, every token draws bars",
  ).toBeGreaterThanOrEqual(TOKENS);

  // There must be something in view, or "nothing in view is missing" is empty.
  expect(
    atRest.inView,
    "the camera must be looking at tokens for the check to mean anything",
  ).toBeGreaterThan(0);

  // Most of the board is off-screen at this size, so most of it is culled.
  // Stated against the board rather than as a count: the view's size is the
  // host's business, and at the default 3,200 it shows a few percent of it.
  if (TOKENS >= 1_600) {
    expect(
      on.withBars,
      "on a board many times the size of the view, most tokens carry no bars",
    ).toBeLessThan(TOKENS / 2);
    expect(
      on.sprites,
      "and the engine holds correspondingly fewer sprites",
    ).toBeLessThan(off.sprites - TOKENS);
  }

  // The gate on speed, and deliberately a weak one: culling must not make the
  // board slower. Frame time here is quantised by vsync, so a level lands on a
  // whole refresh interval and "faster by how much" is the printed figure, not
  // a promise — but slower would mean the bookkeeping costs more than it saves.
  expect(
    on.frameTimeMs,
    "culling on must not cost more per frame than culling off",
  ).toBeLessThanOrEqual(off.frameTimeMs + 1);
});
