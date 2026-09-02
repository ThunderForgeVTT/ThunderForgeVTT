import { expect, test, type Page } from "@playwright/test";
import {
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * How much can the engine actually hold?
 *
 * # What this measures, and what it deliberately does not
 *
 * A sweep: put N tokens on a scene, open it, and read the engine's own
 * counters. `engine_stats()` is mirrored out of the ECS each frame — frame
 * time, fps, and the sprite/token/light/wall/shadow-quad counts — so this
 * asks the engine what it is doing rather than timing it from outside.
 *
 * The tokens are created **through the real mutation path** and loaded the
 * way the application loads any scene. There is a faster way — push commands
 * straight into the wasm and skip the server — and it is deliberately not
 * used. `probe.ts` states the principle it would violate: "a debugging
 * surface that can also mutate state becomes a way to write tests that pass
 * against situations the app cannot actually reach." A number produced by a
 * path no user takes is not a number about the product.
 *
 * # Why the page reloads between levels
 *
 * Measuring after incrementally adding tokens to a live scene would fold the
 * cost of *arrival* — sync events, refetches, sprite spawning — into a figure
 * meant to describe steady state. Each level gets a fresh load, so what is
 * reported is what a player sees on opening a world that size.
 *
 * # These are measurements first and a gate second
 *
 * The assertion at the end is deliberately loose: it fails only if the engine
 * stops rendering usefully at a size a real table might reach. The point is
 * the recorded curve — where frame time starts climbing is worth knowing long
 * before anything breaks, and this is the harness that can answer it.
 */

/**
 * Token counts to sweep.
 *
 * The first run of this stopped at 400 and found nothing: a flat 60fps and a
 * frame time pinned at 16.7ms from 25 tokens all the way up. That is vsync,
 * not the engine — the loop finishes early and waits for the display, so
 * every level below the knee reports the same number and the interesting
 * question goes unanswered. These levels are chosen to find where frame time
 * first exceeds the vsync interval, because that is the only point at which
 * the engine is telling us something about itself rather than about the
 * monitor.
 */
const LEVELS = [3200, 4000, 4800, 5600, 6400];

/** Seconds of sampling per level, after the scene has settled. */
const SAMPLE_MS = 4_000;

interface Sample {
  tokens: number;
  fps: number;
  frameTimeMs: number;
  sprites: number;
  shadowQuads: number;
}

/** Read the engine's own counters, or null if it is not mounted. */
async function readStats(page: Page) {
  return page.evaluate(async () => {
    const mod = (await import(
      /* @vite-ignore */ "/src/engine/bevy/stats.ts"
    )) as typeof import("../src/engine/bevy/stats");
    return mod.readEngineStats();
  });
}

/**
 * Sample for a while and take the median frame time.
 *
 * The median rather than the mean, because one stalled frame — a texture
 * upload, a GC — would drag a mean somewhere that describes the stall rather
 * than the steady state. The stalls are worth knowing about separately; that
 * is what `frame_trace()` is for.
 */
async function sampleSteadyState(page: Page, tokens: number): Promise<Sample> {
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
    tokens,
    fps: Math.round(mid.fps),
    frameTimeMs: Number(mid.frameTimeMs.toFixed(2)),
    sprites: last?.sprites ?? 0,
    shadowQuads: last?.shadowQuads ?? 0,
  };
}

/** Create `count` tokens on a scene, straight through the real mutation. */
async function addTokens(
  page: Page,
  sceneId: string,
  count: number,
  alreadyThere: number,
): Promise<void> {
  await page.evaluate(
    async ({ scene, howMany, offset }) => {
      const csrf = document.cookie
        .split(";")
        .map((part) => part.trim())
        .find((part) => part.startsWith("csrf_token="))
        ?.slice("csrf_token=".length);

      // Spread across a grid rather than stacked on the origin. A pile of
      // coincident sprites is not the same rendering problem as a populated
      // map — overdraw, culling and the shadow pass all behave differently.
      const side = Math.ceil(Math.sqrt(howMany + offset));
      const create = async (i: number) => {
        const n = i + offset;
        const x = ((n % side) - side / 2) * 140;
        const y = (Math.floor(n / side) - side / 2) * 140;
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
            variables: { input: { sceneId: scene, x, y } },
          }),
        });
        const body = await res.json();
        if (body.errors) {
          throw new Error(`createToken failed: ${JSON.stringify(body.errors)}`);
        }
      };

      // In flights rather than one at a time. Thousands of sequential round
      // trips is most of this test's wall clock, and the browser caps
      // HTTP/1.1 at six per host anyway — asking for more just queues, it
      // does not overwhelm anything.
      const IN_FLIGHT = 12;
      for (let start = 0; start < howMany; start += IN_FLIGHT) {
        await Promise.all(
          Array.from({ length: Math.min(IN_FLIGHT, howMany - start) }, (_, k) =>
            create(start + k),
          ),
        );
      }
    },
    { scene: sceneId, howMany: count, offset: alreadyThere },
  );
}

test("engine capacity sweep: frame time against token count", async ({
  page,
}) => {
  // This is a measurement run, not a unit test. Creating 400 tokens through
  // the real mutation path is most of the wall clock.
  test.setTimeout(30 * 60_000);

  const worldId = await registerAndCreateWorld(
    page,
    `Engine Limits ${uniqueSuffix()}`,
  );
  const [sceneId] = await sceneIds(page, worldId);

  const samples: Sample[] = [];
  let created = 0;

  for (const level of LEVELS) {
    await addTokens(page, sceneId, level - created, created);
    created = level;

    // A fresh load, so the figure describes opening a world of this size
    // rather than the cost of tokens arriving one at a time.
    await page.goto(`/world/${worldId}/play`);
    await waitForEngineReady(page);
    // Let the scene settle: the loaders and the first sprite spawns are not
    // steady state and would flatter or damn the number depending on timing.
    await page.waitForTimeout(4_000);

    const sample = await sampleSteadyState(page, level);
    samples.push(sample);

    // table it prints; a passing assertion says far less than the curve.
    console.log(
      `[engine] tokens=${String(sample.tokens).padStart(4)} ` +
        `fps=${String(sample.fps).padStart(4)} ` +
        `frame=${String(sample.frameTimeMs).padStart(7)}ms ` +
        `sprites=${sample.sprites} shadowQuads=${sample.shadowQuads}`,
    );
  }

  console.log(`[engine] sweep=${JSON.stringify(samples)}`);

  // Every level must have produced a real reading. A zero here means the
  // engine never reported, which would make the whole table meaningless —
  // and is a different failure from "the engine was slow".
  expect(
    samples.filter((sample) => sample.fps <= 0),
    "every level must yield a real frame-rate reading from the engine",
  ).toEqual([]);

  // The gate, deliberately loose. A table of 100 tokens is a large but
  // reachable battle map; if the engine cannot hold an interactive frame rate
  // there, that is a product problem rather than a slow test machine.
  const reachable = samples[0];
  expect(
    reachable.fps,
    "the lowest level swept is far past a real battle map — the engine must stay " +
      "interactive there",
  ).toBeGreaterThan(20);
});
