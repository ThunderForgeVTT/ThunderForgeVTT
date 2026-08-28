/**
 * The engine's own performance counters, for the canvas readout.
 *
 * # Why this does not go through `index.ts`
 *
 * `index.ts` owns *mounting* the engine — a module-level load promise, the
 * progress reporting, the world-store bridge. Reading a counter needs none
 * of that, and routing through it would mean a diagnostics readout could
 * start a ~190MB wasm load on a page that never wanted a canvas. Importing
 * the module here is free: ES module instances are shared, so this reaches
 * the same instance `index.ts` initialised, and never initialises one
 * itself.
 *
 * The consequence, and it is the right one: if the engine has not been
 * mounted, this reports `null` rather than mounting it.
 */

/** What `engine_stats()` mirrors out of the ECS each frame. */
export interface EngineStats {
  frameTimeMs: number;
  fps: number;
  sprites: number;
  tokens: number;
  lights: number;
  walls: number;
  shadowQuads: number;
}

type StatsModule = {
  engine_stats?: () => string;
};

let module: StatsModule | null = null;
let loadFailed = false;

async function statsModule(): Promise<StatsModule | null> {
  if (module || loadFailed) return module;
  try {
    module = (await import("@thunderforge/engine/engine")) as StatsModule;
  } catch {
    // An engine bundle that cannot be imported is not an error worth
    // surfacing from a readout — the canvas will have reported it already.
    loadFailed = true;
  }
  return module;
}

/**
 * The latest counters, or `null` when there are none to be had.
 *
 * `null` covers three cases that are the same to a caller: the engine is
 * not mounted, the bundle predates `engine_stats`, or the call threw.
 * Every one of them means "draw no numbers", never "draw zero" — a readout
 * showing a confident `0 fps` over a canvas that is animating happily is
 * worse than showing nothing.
 */
export async function readEngineStats(): Promise<EngineStats | null> {
  const engine = await statsModule();
  if (!engine?.engine_stats) return null;
  try {
    const raw = JSON.parse(engine.engine_stats()) as Record<string, unknown>;
    const fps = typeof raw.fps === "number" ? raw.fps : null;
    const frameTimeMs =
      typeof raw.frame_time_ms === "number" ? raw.frame_time_ms : null;
    if (fps === null || frameTimeMs === null) return null;
    const count = (value: unknown) => (typeof value === "number" ? value : 0);
    return {
      fps,
      frameTimeMs,
      sprites: count(raw.sprites),
      tokens: count(raw.tokens),
      lights: count(raw.lights),
      walls: count(raw.walls),
      shadowQuads: count(raw.shadow_quads),
    };
  } catch {
    return null;
  }
}

/** Reset module state. Tests only. */
export function resetEngineStatsForTests(): void {
  module = null;
  loadFailed = false;
}
