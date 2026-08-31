/**
 * Reading back what the engine holds and what it has dispatched.
 *
 * Spec 030. Three questions the engine is the only thing that can answer:
 *
 * - **Which walls are drawn.** "A player is not shown a secret door" is a
 *   claim about drawing and nothing else can settle it. The geometry is
 *   deliberately sent to every client — a wall that did not arrive would also
 *   stop blocking vision — so a payload check would prove the opposite of what
 *   the story claims, and a screenshot proves only that something was painted.
 * - **Which interactives it holds**, as this viewer received them.
 * - **What it has dispatched**, which is how the seam itself is observed: a
 *   contributor's *result* is visible on the canvas, but that an activation
 *   reached the message at all is only visible here — and that is exactly the
 *   difference needed when a contributor is deliberately absent.
 *
 * # Why this module exists rather than importing the engine inline
 *
 * A bare `@thunderforge/engine/engine` specifier does not resolve inside
 * `page.evaluate`, and `/@fs/` resolves to a *second, uninitialised* wasm
 * instance — which reads as "the engine holds nothing" rather than as a
 * resolution mistake. Only a module under `/src/` reaches the instance the
 * application actually mounted. That cost spec 029 three attempts to find.
 *
 * Read-only, deliberately, following `probe.ts`: an observation surface that
 * can also mutate becomes a way to write tests that pass against situations
 * the application cannot reach.
 */

type EngineModule = {
  drawn_wall_ids?: () => string;
  list_interactives?: () => string;
  dispatched_effects?: () => string;
};

let engineModule: EngineModule | null = null;
let loadFailed = false;

async function engine(): Promise<EngineModule | null> {
  if (engineModule || loadFailed) return engineModule;
  try {
    engineModule =
      (await import("@thunderforge/engine/engine")) as EngineModule;
  } catch {
    loadFailed = true;
  }
  return engineModule;
}

function parse<T>(raw: string | undefined, fallback: T): T {
  if (!raw) return fallback;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

/** Which walls the engine is currently drawing. */
export async function drawnWallIds(): Promise<string[]> {
  const module = await engine();
  // An empty list when the engine is not mounted, not a throw: the caller is
  // asking what is on screen, and "nothing" is the honest answer.
  return parse(module?.drawn_wall_ids?.(), [] as string[]);
}

/** What the engine holds for each interactive, as this viewer received it. */
export async function heldInteractives(): Promise<Record<string, unknown>[]> {
  const module = await engine();
  return parse(module?.list_interactives?.(), [] as Record<string, unknown>[]);
}

/** Every effect the engine has dispatched this session, in order. */
export async function dispatchedEffects(): Promise<Record<string, unknown>[]> {
  const module = await engine();
  return parse(module?.dispatched_effects?.(), [] as Record<string, unknown>[]);
}
