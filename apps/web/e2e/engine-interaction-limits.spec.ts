import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 030, SC-007 — what does a scene full of interactives cost?
 *
 * The expected answer is "nothing measurable", and that is exactly why this
 * exists. An expected result that was never checked is an assumption, and this
 * one has a plausible way of being wrong: entry detection runs every frame
 * over every token, and a naive implementation would turn a populated board
 * into a quadratic sweep without anybody noticing until a real table hit it.
 *
 * # Why both sides are measured in one session
 *
 * A committed baseline was recorded on a different run, possibly different
 * hardware, certainly a different engine build. Reading a cost out of the gap
 * between a fresh number and a stale one measures the gap between the runs.
 * So the same board is measured twice, back to back, with the interactives the
 * only thing that changed — the methodology `engine-status-limits.spec.ts`
 * established and for the same reason.
 *
 * # Why the tokens are moved rather than left standing
 *
 * A still board never exercises entry detection at all: the system compares
 * previous against current position and returns immediately when they match.
 * Measuring a static scene would report zero cost for a feature that had not
 * run, which is the most flattering possible measurement and the least honest
 * one. So the board is put in play and the tokens are moved while sampling.
 */

/** How many interactives a scene is expected to reach (SC-007). */
const INTERACTIVES = Number(process.env.INTERACTION_COUNT ?? "50");

/** Tokens on the board, all of them moving. */
const TOKENS = Number(process.env.INTERACTION_TOKENS ?? "200");

/** Seconds of sampling per condition, after the scene has settled. */
const SAMPLE_MS = 4_000;

/** Settle window before sampling — loaders and first spawns are not steady state. */
const SETTLE_MS = 4_000;

/**
 * How much slower the interactive board may be before this fails.
 *
 * Generous on purpose. The claim is "no measurable change", and a threshold
 * tight enough to catch a 5% difference would mostly catch the noise between
 * two four-second samples on a shared machine. What this is built to catch is
 * an order-of-magnitude mistake — a per-frame sweep that scales with tokens
 * times regions — and that shows up far past 25%.
 */
const TOLERATED_SLOWDOWN = 1.25;

interface Sample {
  condition: string;
  fps: number;
  frameTimeMs: number;
  sprites: number;
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

async function readStats(page: Page) {
  try {
    return await page.evaluate(async () => {
      const mod = (await import(
        /* @vite-ignore */ "/src/engine/bevy/stats.ts"
      )) as typeof import("../src/engine/bevy/stats");
      return mod.readEngineStats();
    });
  } catch {
    // A reload lands here as "execution context was destroyed". Losing a few
    // readings is what the sample count is for; losing the run is not.
    return null;
  }
}

/**
 * Sample while the board is moving, and take the median frame time.
 *
 * Median rather than mean, the same reduction the other capacity specs use: a
 * single stalled frame drags a mean somewhere that describes the stall.
 */
async function sampleWhileMoving(
  page: Page,
  condition: string,
  sceneId: string,
  tokenIds: string[],
): Promise<Sample> {
  const readings: { fps: number; frameTimeMs: number }[] = [];
  let last = await readStats(page);
  const deadline = Date.now() + SAMPLE_MS;
  let step = 0;

  while (Date.now() < deadline) {
    // Move every token a little, through the store the engine listens to. This
    // is what makes entry detection actually run — a still board returns from
    // it immediately.
    await page
      .evaluate(
        async (args: { scene: string; tokens: string[]; step: number }) => {
          const bevy = (await import(
            /* @vite-ignore */ "/src/engine/bevy/index.ts"
          )) as typeof import("../src/engine/bevy/index");
          const store = bevy.getBoundWorldStore();
          if (!store) return;
          const side = Math.ceil(Math.sqrt(args.tokens.length));
          args.tokens.forEach((id, index) => {
            const x = ((index % side) - side / 2) * 140 + args.step * 37;
            const y = (Math.floor(index / side) - side / 2) * 140;
            store.dispatch(
              { type: "upsert_token", token: { id, x, y, z: 0 } },
              "sync",
            );
          });
        },
        { scene: sceneId, tokens: tokenIds, step },
      )
      .catch(() => {
        // Same reasoning as `readStats`: a reload must not lose the run.
      });
    step += 1;

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
    fps: Math.round(mid.fps),
    frameTimeMs: Number(mid.frameTimeMs.toFixed(2)),
    sprites: last?.sprites ?? 0,
    samples: readings.length,
  };
}

test("a scene with fifty interactives costs no measurable frame time", async ({
  page,
}) => {
  test.setTimeout(10 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Capacity ${suffix}`);

  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    page,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  // --- the board ---------------------------------------------------------

  const side = Math.ceil(Math.sqrt(TOKENS));
  const tokenIds: string[] = [];
  for (let index = 0; index < TOKENS; index += 1) {
    const created = await gql<{ createToken: { tokenId: string } }>(
      page,
      `mutation ($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) { tokenId }
      }`,
      {
        input: {
          sceneId,
          x: ((index % side) - side / 2) * 140,
          y: (Math.floor(index / side) - side / 2) * 140,
          tokenType: "character",
        },
      },
    );
    tokenIds.push(created.createToken.tokenId);
  }

  // --- condition one: no interactives -------------------------------------

  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);
  await page.waitForTimeout(SETTLE_MS);
  const absent = await sampleWhileMoving(page, "absent", sceneId, tokenIds);

  // --- fifty interactives, all of them regions ----------------------------

  // Regions rather than props, deliberately: a prop's interactive costs one
  // map entry and nothing per frame, so a board of props would measure almost
  // nothing. Regions are the shape with a per-frame cost, so fifty of them is
  // the worst case fifty interactives can be.
  const interactiveIds: string[] = [];
  for (let index = 0; index < INTERACTIVES; index += 1) {
    const created = await gql<{
      createInteractive: { interactiveId: string };
    }>(
      page,
      `mutation ($input: GraphQLCreateInteractiveInput!) {
        createInteractive(input: $input) { interactiveId }
      }`,
      {
        input: {
          sceneId,
          subjectKind: "region",
          geometry: {
            shape: "rect",
            x: ((index % side) - side / 2) * 140,
            y: (Math.floor(index / side) - side / 2) * 140,
            width: 120,
            height: 120,
          },
          trigger: "enter",
          activation: "gm_only",
          fireMode: "always",
        },
      },
    );
    interactiveIds.push(created.createInteractive.interactiveId);
  }

  // Reload so the figure describes steady state rather than arrival, then put
  // the scene in play — preparation short-circuits detection entirely, and
  // measuring that would measure nothing.
  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);
  const held = await page.evaluate(async (scene: string) => {
    const sync = (await import(
      /* @vite-ignore */ "/src/engine/world/sync/interactives.ts"
    )) as typeof import("../src/engine/world/sync/interactives");
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    const probe = (await import(
      /* @vite-ignore */ "/src/engine/bevy/interactionProbe.ts"
    )) as typeof import("../src/engine/bevy/interactionProbe");

    const store = bevy.getBoundWorldStore();
    if (!store) return 0;
    await sync.refreshInteractives(store, scene);
    sync.setScenePlaying(store, true);
    await new Promise((resolve) => setTimeout(resolve, 600));
    return (await probe.heldInteractives()).length;
  }, sceneId);

  // The measurement is worthless if the engine did not actually receive them.
  // A figure taken from an empty engine would report "no cost" perfectly.
  expect(held, "the engine holds every interactive being measured").toBe(
    INTERACTIVES,
  );

  await page.waitForTimeout(SETTLE_MS);
  const present = await sampleWhileMoving(page, "present", sceneId, tokenIds);

  // --- the answer ---------------------------------------------------------

  const ratio =
    absent.frameTimeMs > 0 ? present.frameTimeMs / absent.frameTimeMs : 1;

  console.log(
    `[interaction-capacity] tokens=${TOKENS} interactives=${INTERACTIVES} ` +
      `absent=${absent.frameTimeMs}ms/${absent.fps}fps(n=${absent.samples}) ` +
      `present=${present.frameTimeMs}ms/${present.fps}fps(n=${present.samples}) ` +
      `ratio=${ratio.toFixed(3)}`,
  );

  expect(
    absent.samples,
    "a thin sample is a weak figure, whichever way it points",
  ).toBeGreaterThan(5);
  expect(present.samples).toBeGreaterThan(5);

  expect(
    ratio,
    `fifty interactives cost ${((ratio - 1) * 100).toFixed(1)}% of frame time`,
  ).toBeLessThan(TOLERATED_SLOWDOWN);
});
