import type { WorldCacheSyncSummary } from "@/engine/bevy";

/**
 * The last world-open sync summary, held for the diagnostics panel
 * (spec 028 FR-051, T122).
 *
 * # Why a service rather than component state
 *
 * The sync runs once, early, from `WorldPage`'s open effect — long before
 * anybody opens a diagnostics panel, and it does not run again when they do.
 * A panel that read the summary from a prop would therefore be empty exactly
 * when it is looked at, and the obvious fix — re-running the sync so the
 * panel has something to show — would make opening a readout do real work to
 * the cache. Holding the summary the open already produced costs nothing and
 * cannot perturb what it reports on.
 *
 * # This is where repairs and evictions come from
 *
 * `canvas_asset_origins()` knows where bytes came from; only the sync knows
 * what the store had to *repair* — the index rows dropped and blobs
 * reclaimed by the FR-019 pass — and what the FR-022 budget evicted. FR-051
 * asks the diagnostics view for both, so both have to reach it, and this is
 * the whole of the plumbing.
 *
 * # Counts, never content
 *
 * Whatever `WorldCacheSyncSummary` carries and nothing more: numbers and a
 * status, never which items. That restraint is the engine's (see
 * `WorldCacheSyncSummary`'s own doc), and this only forwards it. Nothing here
 * is sent anywhere — FR-052/FR-054.
 */

/**
 * Re-exported so a reader of these figures does not have to import from
 * `@/engine/bevy` — a module whose other exports mount a ~190MB wasm bundle.
 * The type is erased at build time either way; keeping the import out of the
 * panel keeps the temptation out with it.
 */
export type { WorldCacheSyncSummary };

type Listener = (summary: WorldCacheSyncSummary | null) => void;

const listeners = new Set<Listener>();

let latest: WorldCacheSyncSummary | null = null;

/** The most recent sync summary, for a caller that does not want to subscribe. */
export function getWorldCacheSync(): WorldCacheSyncSummary | null {
  return latest;
}

/**
 * Record what a world open's sync did. Called by `WorldPage`.
 *
 * A `null` — the engine has no cache entry point at all — is stored as a
 * `null` rather than ignored, because "this browser is running without the
 * cache" is a true and useful thing for the panel to be able to say.
 */
export function reportWorldCacheSync(
  summary: WorldCacheSyncSummary | null,
): void {
  latest = summary;
  for (const listener of listeners) listener(latest);
}

/** Observe the latest summary. Returns an unsubscribe. */
export function subscribeToWorldCacheSync(listener: Listener): () => void {
  listeners.add(listener);
  listener(latest);
  return () => {
    listeners.delete(listener);
  };
}

/** Reset module state. Tests only. */
export function resetWorldCacheDiagnosticsForTests(): void {
  listeners.clear();
  latest = null;
}
