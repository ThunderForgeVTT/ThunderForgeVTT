/**
 * Knowing the engine has died (spec 070).
 *
 * Two things end a running board, and neither used to reach the page:
 *
 * - **The engine panics.** In wasm that is a trap — the frame loop never runs
 *   again and every later call into the module throws. The engine's panic
 *   hook sends `thunderforge:engine-stopped` to the window as it goes.
 * - **The browser takes the graphics context away** — a driver reset, a
 *   laptop switching GPUs, a phone reclaiming memory from a background tab.
 *   The canvas fires `webglcontextlost`. A browser may hand a context back
 *   later; the engine cannot pick it up, so this is as final as a panic.
 *
 * Either way the canvas keeps its last picture, which is exactly what a
 * working board looks like until somebody tries to move something.
 *
 * Module state rather than a hook's: the engine is one per page and outlives
 * the component that shows it, so its death has to as well. There is no way
 * back to "running" but a reload, which clears this with everything else.
 */
import { useSyncExternalStore } from "react";

export type EngineStopReason = "crashed" | "context-lost";

/** The engine's own name for the event; `startup.rs` sends it. */
export const ENGINE_STOPPED_EVENT = "thunderforge:engine-stopped";

let reason: EngineStopReason | null = null;
let watching = false;
const listeners = new Set<() => void>();

function stop(why: EngineStopReason): void {
  // The first reason is the reason: a panic is often followed by a lost
  // context as the tab is torn down, and that says nothing new.
  if (reason !== null) return;
  reason = why;
  for (const listener of listeners) listener();
}

/**
 * Start listening. Idempotent, and never undone: the listeners are two, they
 * are on `window`, and a board that died while nothing was mounted to hear it
 * still died.
 */
export function watchEngineStopped(
  target: EventTarget | undefined = globalThis.window,
): void {
  if (watching || !target) return;
  watching = true;
  target.addEventListener(ENGINE_STOPPED_EVENT, () => stop("crashed"));
  // Captured on the way down, so it is heard whatever the canvas — which is
  // the engine's element, not React's — does with it. Not asked whose canvas:
  // the board's is the only WebGL context this app makes.
  target.addEventListener("webglcontextlost", () => stop("context-lost"), true);
}

/** Why the engine stopped, or `null` while it has not. */
export function engineStopReason(): EngineStopReason | null {
  return reason;
}

export function subscribeEngineStopped(listener: () => void): () => void {
  watchEngineStopped();
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** [`engineStopReason`], for a component. */
export function useEngineStopped(): EngineStopReason | null {
  return useSyncExternalStore(
    subscribeEngineStopped,
    engineStopReason,
    () => null,
  );
}

/** Tests only: a page reload is the only thing that does this for real. */
export function resetEngineStoppedForTests(): void {
  reason = null;
  watching = false;
  listeners.clear();
}
