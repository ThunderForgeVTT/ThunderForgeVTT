/**
 * Where this session's canvas-asset bytes came from (spec 028 FR-051).
 *
 * # Why this sits beside `stats.ts` and not in `index.ts`
 *
 * The same reason `stats.ts` gives, and it matters more here. `index.ts` owns
 * *mounting* the engine; a readout only wants to read a counter. Routing
 * through it would let a diagnostics panel start a ~190MB wasm download on a
 * page that never asked for a canvas. Importing the wasm module directly is
 * free — ES module instances are shared, so this reaches the instance
 * `index.ts` already initialised and never initialises one itself.
 *
 * The consequence is the honest one: with no engine mounted this reports
 * `null`, and the panel says it has nothing to report rather than drawing a
 * confident row of zeroes.
 *
 * # Counts, never content
 *
 * Counters, and nothing but. The Rust side deliberately assembles nothing else — no
 * asset ids, no fingerprints, no urls — because FR-052/FR-054 say this
 * information stays on the user's machine, and the surest way to keep that
 * promise is for the interesting thing to not exist in the first place.
 */

/** What `canvas_asset_origins()` reports: a pair per `Origin`, plus prefetch. */
export interface CacheOriginTally {
  /** Served from this device's encrypted store — the bytes never fetched. */
  cacheItems: number;
  cacheBytes: number;
  /** Fetched from the server and verified against its own promise. */
  networkItems: number;
  networkBytes: number;
  /** Supplied by another client and verified before use (FR-046). */
  peerItems: number;
  peerBytes: number;
  /**
   * Fetched, but the bytes did not match the fingerprint the server
   * promised. Rendered, never stored (see `fetch_and_deliver`).
   */
  unverifiedItems: number;
  unverifiedBytes: number;
  /**
   * Brought in ahead of demand and filed, never handed to a caller. Bytes
   * transferred, but not items *served* — which is why they are counted apart
   * from `networkItems` rather than folded into it (see `OriginTally` in
   * `cached_assets.rs`). Ignoring them would make the panel understate what a
   * visit cost, and on a warm world the prefetch is usually what brings a
   * changed asset down.
   */
  prefetchedItems: number;
  prefetchedBytes: number;
  prefetchedPeerItems: number;
  prefetchedPeerBytes: number;
}

type OriginsModule = {
  canvas_asset_origins?: () => string;
};

let module: OriginsModule | null = null;
let loadFailed = false;

async function originsModule(): Promise<OriginsModule | null> {
  if (module || loadFailed) return module;
  try {
    module = (await import("@thunderforge/engine/engine")) as OriginsModule;
  } catch {
    // A bundle that cannot be imported is not an error worth surfacing from a
    // readout — whatever wanted the canvas has already reported it.
    loadFailed = true;
  }
  return module;
}

/**
 * The session's origin tally, or `null` when there is none to be had.
 *
 * `null` covers three cases a caller cannot act on differently: the engine is
 * not mounted, the bundle predates `canvas_asset_origins`, or the call threw.
 * All three mean "say nothing", never "say zero" — a panel reporting `0
 * served locally` on a page whose art came straight off the disk would be
 * answering the question wrongly rather than admitting it has no answer.
 */
export async function readCacheOrigins(): Promise<CacheOriginTally | null> {
  const engine = await originsModule();
  if (!engine?.canvas_asset_origins) return null;
  try {
    const raw = JSON.parse(engine.canvas_asset_origins()) as Record<
      string,
      unknown
    >;
    const count = (value: unknown) => (typeof value === "number" ? value : 0);
    return {
      cacheItems: count(raw.cacheItems),
      cacheBytes: count(raw.cacheBytes),
      networkItems: count(raw.networkItems),
      networkBytes: count(raw.networkBytes),
      peerItems: count(raw.peerItems),
      peerBytes: count(raw.peerBytes),
      unverifiedItems: count(raw.unverifiedItems),
      unverifiedBytes: count(raw.unverifiedBytes),
      prefetchedItems: count(raw.prefetchedItems),
      prefetchedBytes: count(raw.prefetchedBytes),
      prefetchedPeerItems: count(raw.prefetchedPeerItems),
      prefetchedPeerBytes: count(raw.prefetchedPeerBytes),
    };
  } catch {
    return null;
  }
}

/** Reset module state. Tests only. */
export function resetCacheStatsForTests(): void {
  module = null;
  loadFailed = false;
}
