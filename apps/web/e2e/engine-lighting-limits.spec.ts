import path from "node:path";
import { expect, test, type Page } from "@playwright/test";
import {
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { importMapBackground, sceneIds } from "./fixtures/world-cache";

/**
 * The other half of the engine's cost: shadow-casting lights against walls.
 *
 * # Why this is separate from the token sweep, and why it is the interesting one
 *
 * `engine-limits.spec.ts` sweeps token count and finds 60fps holding to
 * roughly 3,200 sprites. Every scene in that sweep reported
 * `shadowQuads=0` — there were no lights and no walls, so the entire
 * lighting and shadow path went unmeasured. Sprites are cheap and largely
 * independent of each other; shadows are neither. Each shadow-casting light
 * must be resolved against the wall set, so the cost is closer to lights ×
 * walls than to either alone, and that product is where a real dungeon map
 * lives.
 *
 * This sweeps both dimensions together, because measuring one with none of
 * the other would repeat the first sweep's mistake in a new place.
 *
 * # What the numbers mean
 *
 * `shadow_quads` is the engine's own count of the geometry the shadow pass
 * produced, reported beside frame time. A scene where that number is zero is
 * not exercising this path at all, whatever else it contains — which is
 * exactly how the token sweep managed to look flat.
 *
 * # The finding: this path is currently unreachable
 *
 * With a real map, 32 shadow-casting lights and 1,600 vision-blocking walls,
 * `shadowQuads` is still zero at every level. The reason is in `darkness.rs`'s
 * own header — the layer is inert while ambient light is `Bright` — and scene
 * ambient is only settable through the engine's `set_ambient_light` command,
 * which nothing in the web application sends. Neither is
 * `set_lighting_overlay`. The lighting subsystem is built, has its own shader,
 * and cannot currently be switched on by the product.
 *
 * So this file measures lights and walls **as geometry**, and pins that zero
 * so the day it changes is the day this test asks to be rewritten.
 *
 * # Why the scene needs a real imported map
 *
 * The first version of this created lights and walls on an empty scene and
 * measured a clean, plausible-looking curve in which `shadowQuads` was zero
 * at every level. `darkness.rs` explains why: the lighting layer is a
 * **map-sized** darkness sheet with light pools cut out and one shadow quad
 * per (light, vision-blocking wall). With no map there is nothing to size
 * that sheet to, so the whole layer never exists and the numbers describe
 * wall geometry instead. The guard at the bottom of this test is what caught
 * that, and it is the reason the guard is there.
 */

/** A real imported map, so the darkness sheet has something to cover. */
const CHAMBER_MAP = path.resolve(
  __dirname,
  "../../../examples/maps/chamber-of-echoing-grief.dd2vtt",
);

/** (lights, walls) pairs, chosen to cross from a lit room into a dungeon. */
const LEVELS: { lights: number; walls: number }[] = [
  { lights: 2, walls: 50 },
  { lights: 4, walls: 200 },
  { lights: 8, walls: 400 },
  { lights: 16, walls: 800 },
  { lights: 32, walls: 1600 },
];

/** Sampling window per level, after the scene has settled. */
const SAMPLE_MS = 4_000;

interface Sample {
  lights: number;
  walls: number;
  fps: number;
  frameTimeMs: number;
  shadowQuads: number;
}

async function readStats(page: Page) {
  return page.evaluate(async () => {
    const mod = (await import(
      /* @vite-ignore */ "/src/engine/bevy/stats.ts"
    )) as typeof import("../src/engine/bevy/stats");
    return mod.readEngineStats();
  });
}

/**
 * Median frame time over the window.
 *
 * The median, not the mean: one stalled frame would drag a mean somewhere
 * that describes the stall rather than the steady state.
 */
async function sampleSteadyState(
  page: Page,
  lights: number,
  walls: number,
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
    lights,
    walls,
    fps: Math.round(mid.fps),
    frameTimeMs: Number(mid.frameTimeMs.toFixed(2)),
    shadowQuads: last?.shadowQuads ?? 0,
  };
}

/** Add shadow-casting lights and vision-blocking walls, the real way. */
async function populate(
  page: Page,
  sceneId: string,
  addLights: number,
  addWalls: number,
  lightOffset: number,
  wallOffset: number,
): Promise<void> {
  await page.evaluate(
    async ({ scene, lights, walls, lightsFrom, wallsFrom }) => {
      const csrf = document.cookie
        .split(";")
        .map((part) => part.trim())
        .find((part) => part.startsWith("csrf_token="))
        ?.slice("csrf_token=".length);

      const send = async (query: string, input: Record<string, unknown>) => {
        const res = await fetch("/api/graphql", {
          method: "POST",
          credentials: "same-origin",
          headers: {
            "Content-Type": "application/json",
            ...(csrf ? { "x-csrf-token": csrf } : {}),
          },
          body: JSON.stringify({ query, variables: { input } }),
        });
        const body = await res.json();
        if (body.errors) throw new Error(JSON.stringify(body.errors));
      };

      const WALL = `mutation ($input: GraphQLCreateWallInput!) {
        createWall(input: $input) { wallId }
      }`;
      const LIGHT = `mutation ($input: GraphQLCreateLightSourceInput!) {
        createLightSource(input: $input) { lightId }
      }`;

      // Walls laid out as a lattice of short segments rather than one long
      // wall. A single segment is one occluder however long it is; a room
      // full of them is what a shadow pass actually has to resolve.
      const wallSide = Math.ceil(Math.sqrt(walls + wallsFrom));
      const wallJobs: (() => Promise<void>)[] = [];
      for (let i = 0; i < walls; i += 1) {
        const n = i + wallsFrom;
        const x = ((n % wallSide) - wallSide / 2) * 200;
        const y = (Math.floor(n / wallSide) - wallSide / 2) * 200;
        wallJobs.push(() =>
          send(WALL, {
            sceneId: scene,
            x1: x,
            y1: y,
            x2: x + 150,
            y2: y,
            blocksVision: true,
          }),
        );
      }

      // Lights spread across the same field, every one casting shadows —
      // `castsShadows: false` would leave this measuring nothing but sprite
      // count again.
      const lightJobs: (() => Promise<void>)[] = [];
      for (let i = 0; i < lights; i += 1) {
        const n = i + lightsFrom;
        const angle = (n / 8) * Math.PI * 2;
        lightJobs.push(() =>
          send(LIGHT, {
            sceneId: scene,
            x: Math.cos(angle) * (300 + n * 40),
            y: Math.sin(angle) * (300 + n * 40),
            radius: 600,
            intensity: 1,
            castsShadows: true,
          }),
        );
      }

      const IN_FLIGHT = 12;
      const jobs = [...wallJobs, ...lightJobs];
      for (let start = 0; start < jobs.length; start += IN_FLIGHT) {
        await Promise.all(
          jobs.slice(start, start + IN_FLIGHT).map((job) => job()),
        );
      }
    },
    {
      scene: sceneId,
      lights: addLights,
      walls: addWalls,
      lightsFrom: lightOffset,
      wallsFrom: wallOffset,
    },
  );
}

test("engine sweep: lights and walls as geometry, with the shadow pass proven inert", async ({
  page,
}) => {
  // A measurement run. Creating thousands of walls through the real mutation
  // path is most of the wall clock.
  test.setTimeout(45 * 60_000);

  const worldId = await registerAndCreateWorld(
    page,
    `Engine Lighting ${uniqueSuffix()}`,
  );
  const [sceneId] = await sceneIds(page, worldId);

  // The map first. It brings its own walls with it, which is realistic — a
  // dd2vtt import is how a dungeon gets its geometry — and it is what gives
  // the darkness layer a size to be.
  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);
  await importMapBackground(page, CHAMBER_MAP);

  const samples: Sample[] = [];
  let lightsMade = 0;
  let wallsMade = 0;

  for (const level of LEVELS) {
    await populate(
      page,
      sceneId,
      level.lights - lightsMade,
      level.walls - wallsMade,
      lightsMade,
      wallsMade,
    );
    lightsMade = level.lights;
    wallsMade = level.walls;

    await page.goto(`/world/${worldId}/play`);
    await waitForEngineReady(page);
    // Lighting settles later than sprites do — the shadow geometry is built
    // from the wall set once it has loaded, not as each wall arrives.
    await page.waitForTimeout(6_000);

    const sample = await sampleSteadyState(page, level.lights, level.walls);
    samples.push(sample);
     
    console.log(
      `[engine] lights=${String(sample.lights).padStart(3)} ` +
        `walls=${String(sample.walls).padStart(5)} ` +
        `fps=${String(sample.fps).padStart(4)} ` +
        `frame=${String(sample.frameTimeMs).padStart(7)}ms ` +
        `shadowQuads=${sample.shadowQuads}`,
    );
  }

   
  console.log(`[engine] lightingSweep=${JSON.stringify(samples)}`);

  expect(
    samples.filter((sample) => sample.fps <= 0),
    "every level must yield a real frame-rate reading from the engine",
  ).toEqual([]);

  // What this sweep actually discovered, pinned so it cannot be forgotten.
  //
  // Every level reports zero shadow quads, on a real imported map, with 32
  // shadow-casting lights and 1,600 vision-blocking walls. `darkness.rs`
  // says why in its own header: "the whole layer is inert while ambient
  // light is Bright". Scene ambient is set by the engine's
  // `set_ambient_light` external command — and **nothing in the web
  // application ever sends it**, nor `set_lighting_overlay`. So the darkness
  // sheet, the light pools cut out of it, and the per-(light, wall) shadow
  // quads are all unreachable from the shipped product, and the numbers
  // above measure lights and walls purely as geometry.
  //
  // Asserted rather than commented, so this test fails the day someone wires
  // ambient up — which is exactly when it should be extended to measure the
  // pass it was originally written for.
  expect(
    samples.every((sample) => sample.shadowQuads === 0),
    "the shadow pass is currently unreachable from the app (scene ambient is " +
      "never set, so darkness.rs stays inert). If this fails, lighting has been " +
      "wired up and this sweep should now measure it.",
  ).toBe(true);

  // A lit room with a few lights and a couple of hundred walls is an
  // ordinary dungeon map, not a stress case.
  const ordinary = samples[1];
  expect(
    ordinary.fps,
    "an ordinary lit map must stay interactive",
  ).toBeGreaterThan(20);
});
