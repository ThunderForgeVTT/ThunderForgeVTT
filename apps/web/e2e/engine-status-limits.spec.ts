import { expect, test, type Browser, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 029, SC-006 — what do in-engine status displays cost in capacity?
 *
 * # Why this is a sibling of `engine-limits.spec.ts` rather than a level in it
 *
 * `engine-limits.spec.ts` sweeps token count on a bare board to find where
 * frame time leaves the vsync floor. This sweeps the same axis twice — once
 * with every token drawing bars, once with none of them drawing anything — so
 * the answer is a capacity with displays on and the capacity it was reduced
 * from, rather than a frame time at one arbitrary size.
 *
 * The methodology is deliberately copied from that file rather than invented:
 * tokens created through the real mutation path, a fresh page load so the
 * figure describes steady state rather than arrival, the same settle window,
 * the same sampling window, the same median-not-mean reduction. A comparison
 * is only worth as much as the sameness of its two sides.
 *
 * # Why both sides are measured here
 *
 * The committed 3,200-sprite baseline was recorded from a different run on
 * possibly different hardware and certainly a different engine build. Reading
 * a "cost" out of the gap between a fresh number and a stale one measures the
 * gap between the runs, not the gap between the features. Both conditions are
 * therefore measured in the same session, back to back.
 *
 * # Why the tokens share one actor
 *
 * Every token gets its own `TokenStatus` component and its own despawn-and-
 * rebuild of bar geometry, so the *rendering* work per token is identical
 * whether the resources came from 3,200 actors or one. What sharing avoids is
 * thousands of extra setup mutations, which would be most of this run's clock
 * and would measure the server rather than the engine. The board this
 * describes is a mob of identical creatures, which is a board real tables
 * reach.
 */

/**
 * The levels swept, in tokens.
 *
 * 3,200 is the anchor: it is where `engine-limits.spec.ts` starts its sweep,
 * so a figure here is comparable to one already written down. The smaller
 * levels exist because a single point is a frame time, not a capacity — the
 * question SC-006 asks is *how many tokens* the engine holds with displays on,
 * and only a curve answers that.
 *
 * `STATUS_CAPACITY_LEVELS` overrides them, for smoke-running the harness at a
 * size that finishes in a minute. A figure quoted anywhere is from the default.
 */
const LEVELS = (process.env.STATUS_CAPACITY_LEVELS ?? "400,800,1600,3200")
  .split(",")
  .map((level) => Number(level.trim()));

/**
 * The frame rate below which a board stops feeling like a board.
 *
 * Frame time on this host is quantised by vsync — a level lands on 16.7ms,
 * 33.3ms or 50ms and nothing between — so "capacity" is read as the largest
 * level still holding a whole refresh interval per frame rather than as a
 * smooth curve crossing a line.
 */
const INTERACTIVE_FPS = 30;

/** Seconds of sampling per condition, after the scene has settled. */
const SAMPLE_MS = 4_000;

/** Settle window before sampling — loaders and first spawns are not steady state. */
const SETTLE_MS = 4_000;

/** How long to let status displays finish arriving before giving up on them. */
const DISPLAY_SETTLE_MS = 120_000;

type Condition = "displays-absent" | "displays-enabled";

interface Sample {
  condition: string;
  displays: boolean;
  tokens: number;
  fps: number;
  frameTimeMs: number;
  sprites: number;
  shadowQuads: number;
  /** How many tokens the engine actually held status for when sampled. */
  displayed: number;
  /** How many readings the poll actually got — a thin sample is a weak figure. */
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

/**
 * Read the engine's own counters, or null if there is nothing to read.
 *
 * `null` also covers the evaluate itself failing. The application reloads its
 * own page under some conditions — a dropped socket, a session heartbeat that
 * comes back wrong — and a reload lands here as "execution context was
 * destroyed". Letting that abort the run would turn an ordinary reconnect into
 * a lost measurement; a poll that misses a few readings is what the sample
 * count below is for.
 */
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

/**
 * How many tokens the engine currently holds status for.
 *
 * The reason this exists: status resolves progressively as a scene loads, and
 * on a large board it is still filling in long after the canvas is up. A frame
 * time sampled at that moment describes a board that is partly bare, and would
 * be reported as the cost of a feature that was mostly not running. The first
 * version of this measurement did exactly that, and produced a "cost" that
 * moved by a factor of thirty between runs.
 */
async function displayedCount(page: Page): Promise<number> {
  try {
    return await page.evaluate(async () => {
      const mod = (await import(
        /* @vite-ignore */ "/src/engine/bevy/tokenStatus.ts"
      )) as typeof import("../src/engine/bevy/tokenStatus");
      return Object.keys(await mod.listTokenStatus()).length;
    });
  } catch {
    return 0;
  }
}

/**
 * Wait until the board has finished filling in.
 *
 * Two things have to settle, and they settle at different times. The engine's
 * status map fills first — that is `displayedCount` — and the bar geometry for
 * those tokens is spawned afterwards, a batch at a time. Sampling between the
 * two reports a board whose tokens all *have* status and mostly do not *draw*
 * it, which is the most flattering possible moment to take the measurement and
 * the least honest.
 *
 * So this waits for the map to be complete and then for the sprite count to
 * stop moving. A plateau rather than an arithmetic target, because the number
 * of bars per token is the feature's business, not this file's.
 */
async function waitForDisplays(page: Page, expected: number): Promise<number> {
  let displayed = 0;
  let sprites = -1;
  let steady = 0;
  const deadline = Date.now() + DISPLAY_SETTLE_MS;
  while (Date.now() < deadline) {
    displayed = await displayedCount(page);
    const stats = await readStats(page);
    const count = stats?.sprites ?? -1;
    steady = count === sprites ? steady + 1 : 0;
    sprites = count;
    // Ten seconds of an unmoving sprite count, with every token already in the
    // status map. Geometry arrives in bursts, so a shorter quiet window would
    // call the gap between two bursts a plateau.
    if (displayed >= expected && steady >= 20) return displayed;
    await page.waitForTimeout(500);
  }
  return displayed;
}

/**
 * Sample for a while and take the median frame time — the same reduction
 * `engine-limits.spec.ts` uses, and for the same reason: one stalled frame
 * would drag a mean somewhere that describes the stall, not the steady state.
 */
async function sampleSteadyState(
  page: Page,
  condition: Condition,
  tokens: number,
  displayed: number,
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
  return {
    condition,
    displays: condition === "displays-enabled",
    tokens,
    fps: Math.round(mid.fps),
    frameTimeMs: Number(mid.frameTimeMs.toFixed(2)),
    sprites: last?.sprites ?? 0,
    shadowQuads: last?.shadowQuads ?? 0,
    displayed,
    samples: readings.length,
  };
}

/**
 * Create `count` tokens on a scene through the real mutation, optionally bound
 * to an actor so the engine has resources to display.
 */
async function addTokens(
  page: Page,
  worldId: string,
  sceneId: string,
  count: number,
  alreadyThere: number,
  actorId: string | null,
): Promise<void> {
  // Populate from a page that is not running the engine. Creating a thousand
  // tokens while the canvas is open means a thousand live-sync arrivals into a
  // running engine, and the application answers that by reloading itself —
  // which lands here as "execution context was destroyed" and loses the run.
  // It would also fold the cost of arrival into a figure meant to describe
  // steady state, which is the thing this file is at pains not to do.
  await page.goto(`/world/${worldId}/staging`);

  // The grid is laid out for the level being built, not for the chunk, so a
  // board grown in pieces occupies the same space as one created at once.
  const side = Math.ceil(Math.sqrt(count + alreadyThere));

  // In chunks, so no single evaluate is long enough for a reload to land in
  // the middle of it and take the whole level with it.
  const CHUNK = 200;
  for (let done = 0; done < count; done += CHUNK) {
    await addTokenChunk(
      page,
      sceneId,
      Math.min(CHUNK, count - done),
      alreadyThere + done,
      side,
      actorId,
    );
  }
}

async function addTokenChunk(
  page: Page,
  sceneId: string,
  count: number,
  alreadyThere: number,
  side: number,
  actorId: string | null,
): Promise<void> {
  await page.evaluate(
    async ({ scene, howMany, offset, side: gridSide, actor }) => {
      const csrf = document.cookie
        .split(";")
        .map((part) => part.trim())
        .find((part) => part.startsWith("csrf_token="))
        ?.slice("csrf_token=".length);

      // Spread across a grid rather than stacked on the origin: a pile of
      // coincident sprites is not the same rendering problem as a populated
      // map, and it is even less so once each token draws bars above itself.
      const create = async (i: number) => {
        const n = i + offset;
        const x = ((n % gridSide) - gridSide / 2) * 140;
        const y = (Math.floor(n / gridSide) - gridSide / 2) * 140;
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
                ...(actor ? { actorId: actor, tokenType: "npc" } : {}),
              },
            },
          }),
        });
        const body = await res.json();
        if (body.errors) {
          throw new Error(`createToken failed: ${JSON.stringify(body.errors)}`);
        }
      };

      const IN_FLIGHT = 12;
      for (let start = 0; start < howMany; start += IN_FLIGHT) {
        await Promise.all(
          Array.from({ length: Math.min(IN_FLIGHT, howMany - start) }, (_, k) =>
            create(start + k),
          ),
        );
      }
    },
    {
      scene: sceneId,
      howMany: count,
      offset: alreadyThere,
      side,
      actor: actorId,
    },
  );
}

/**
 * Sweep one condition: build a world, then grow it through the levels,
 * reloading and sampling at each.
 *
 * Each condition gets its own browser context. Not tidiness: the two sides
 * register their own accounts, and a second registration inside a session that
 * already holds one is a different code path from the one a player takes. A
 * fresh context also means a fresh engine mount, so neither side inherits the
 * other's warmed caches.
 *
 * Within a condition the world is grown rather than rebuilt, exactly as
 * `engine-limits.spec.ts` does, and each level still gets a fresh page load —
 * so what is reported is opening a board of that size, never the cost of
 * tokens arriving one at a time.
 */
async function sweep(
  browser: Browser,
  condition: Condition,
): Promise<Sample[]> {
  const context = await browser.newContext();
  const page = await context.newPage();
  try {
    return await sweepIn(page, condition);
  } finally {
    await context.close();
  }
}

/** Set the world up so its tokens have resources to display, or leave it bare. */
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

async function sweepIn(page: Page, condition: Condition): Promise<Sample[]> {
  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(
    page,
    `Status Limits ${condition} ${suffix}`,
  );
  const [sceneId] = await sceneIds(page, worldId);

  const actorId =
    condition === "displays-enabled"
      ? await displayableActor(page, worldId, suffix)
      : null;

  const samples: Sample[] = [];
  let created = 0;
  for (const level of LEVELS) {
    const started = Date.now();
    await addTokens(page, worldId, sceneId, level - created, created, actorId);
    created = level;
    const populatedInMs = Date.now() - started;

    // Load, settle, sample — and be willing to do it again. The application
    // reloads its own page when the live connection drops, which lands in the
    // middle of a sampling window as a run of missed readings. That is a fact
    // about the app, not about the engine's frame time, so a window that lost
    // its page is retried rather than reported.
    let sample: Sample | null = null;
    for (let attempt = 1; attempt <= 3 && !sample; attempt += 1) {
      await page.goto(`/world/${worldId}/play`);
      await waitForEngineReady(page);
      await page.waitForTimeout(SETTLE_MS);
      // Only the enabled side has displays to wait for; asking the bare board
      // to reach a count it will never reach would spend two minutes proving
      // that zero is zero.
      const displayed =
        condition === "displays-enabled"
          ? await waitForDisplays(page, level)
          : 0;
      const attempted = await sampleSteadyState(
        page,
        condition,
        level,
        displayed,
      );
      if (attempted.samples > 5) sample = attempted;
      else
        console.log(
          `[status-capacity] ${condition} tokens=${level} attempt=${attempt} ` +
            `discarded: only ${attempted.samples} readings`,
        );
    }
    if (!sample) {
      throw new Error(
        `${condition} at ${level} tokens: the page never held still long ` +
          `enough to sample`,
      );
    }

    // This run's whole product is the table it prints; the assertions below
    // say far less than the curve.
    console.log(
      `[status-capacity] condition=${condition.padEnd(16)} ` +
        `tokens=${String(sample.tokens).padStart(4)} ` +
        `fps=${String(sample.fps).padStart(3)} ` +
        `frame=${String(sample.frameTimeMs).padStart(7)}ms ` +
        `sprites=${String(sample.sprites).padStart(5)} ` +
        `displaying=${String(sample.displayed).padStart(4)} ` +
        `samples=${sample.samples} populatedInMs=${populatedInMs}`,
    );
    samples.push(sample);
  }
  return samples;
}

/**
 * The largest level that still held an interactive frame rate, or 0.
 *
 * The *largest* rather than the last one before a dip: frame time here is
 * quantised by vsync and a level can land a step either side of a boundary
 * between runs, so a strict "first failure" reading would report a capacity
 * that moves for reasons that have nothing to do with the engine.
 */
function capacity(samples: Sample[]): number {
  const held = samples.filter((sample) => sample.fps >= INTERACTIVE_FPS);
  return held.length ? Math.max(...held.map((sample) => sample.tokens)) : 0;
}

test("status display capacity: with displays and without, same board", async ({
  browser,
}) => {
  // A measurement run, not a unit test. Creating the tokens through the real
  // mutation path is most of the wall clock.
  test.setTimeout(60 * 60_000);

  const absent = await sweep(browser, "displays-absent");
  const enabled = await sweep(browser, "displays-enabled");

  const anchor = LEVELS[LEVELS.length - 1];
  const at = (samples: Sample[]) =>
    samples.find((sample) => sample.tokens === anchor);

  console.log(
    `[status-capacity] result=${JSON.stringify({
      levels: LEVELS,
      interactiveFps: INTERACTIVE_FPS,
      absent,
      enabled,
      capacityAbsent: capacity(absent),
      capacityEnabled: capacity(enabled),
      anchor,
      anchorSpriteDelta:
        (at(enabled)?.sprites ?? 0) - (at(absent)?.sprites ?? 0),
      anchorFrameTimeDeltaMs: Number(
        (
          (at(enabled)?.frameTimeMs ?? 0) - (at(absent)?.frameTimeMs ?? 0)
        ).toFixed(2),
      ),
    })}`,
  );

  const all = [...absent, ...enabled];

  // A zero here means the engine never reported, which is a different failure
  // from "the engine was slow" and would make the comparison meaningless.
  expect(
    all.filter((sample) => sample.fps <= 0),
    "every level must yield a real frame-rate reading from the engine",
  ).toEqual([]);

  // A median over two readings is not a steady state, it is an anecdote. The
  // poll runs at 200ms over the sampling window, so a healthy run collects
  // most of it; a floor keeps a page that reloaded through the whole window
  // from being reported as a measurement.
  expect(
    Math.min(...all.map((sample) => sample.samples)),
    "each level needs enough readings for a median to mean anything",
  ).toBeGreaterThan(5);

  // The displays must actually be on in the enabled condition. Without this
  // the run could report a reassuring "no cost" for a feature that never drew.
  expect(
    at(enabled)?.sprites ?? 0,
    "the enabled condition must draw more sprites than the bare board — " +
      "otherwise no status geometry existed and the comparison is empty",
  ).toBeGreaterThan(at(absent)?.sprites ?? 0);

  // And on for *every* token, not a fraction of them. A frame time taken while
  // the board is still filling in is a measurement of the fill, and reporting
  // it as capacity would be the unmeasured claim SC-006 exists to prevent.
  expect(
    enabled.filter((sample) => sample.displayed < sample.tokens),
    "every level must have every token displaying before it is sampled",
  ).toEqual([]);

  // The gate, and deliberately the only one on speed: the engine must stay
  // interactive with displays on at a size a real table can actually reach.
  // The lowest level swept is already several times a large battle map, so a
  // failure here is a product problem rather than a slow test machine — and
  // the levels above it are the recorded curve, not a promise.
  expect(
    enabled[0].fps,
    "the engine must stay interactive with status displays on at a size a " +
      "real table reaches",
  ).toBeGreaterThanOrEqual(INTERACTIVE_FPS);
});
