import type { EngineMountOptions, EngineState } from "./types";
import type { WorldStore } from "../world/store";
import type { WorldCommand } from "../world/types";

type BevyWasmModule = {
  default: (moduleOrPath?: unknown) => Promise<unknown>;
  start: (canvasSelector: string) => void;
  apply_world_command?: (json: string) => void;
  set_event_callback?: (callback: (json: string) => void) => void;
  /**
   * Spec 028 (US1, T027/T028). Optional because an engine bundle built
   * before this existed must still mount — a missing cache is the
   * degradation this whole path is designed around, not an error.
   */
  sync_world_cache?: (worldId: string, userId: string) => Promise<string>;
  /**
   * Spec 028 (US7, T072). Optional for the same reason `sync_world_cache`
   * is: a bundle built before the outbox existed must still mount, and the
   * caller treats its absence as "cannot queue" rather than as an error.
   */
  queue_offline_change?: (
    worldId: string,
    localId: string,
    command: string,
    isGameMaster: boolean,
  ) => Promise<string>;
  read_queued_changes?: (worldId: string) => Promise<string>;
  forget_reconciled_changes?: (outcomesJson: string) => Promise<string>;
};

let loadPromise: Promise<BevyWasmModule> | null = null;
let worldStoreUnsubscribe: (() => void) | null = null;
let bevyCallbackRegistered = false;
let boundWorldStore: WorldStore | null = null;

const state: EngineState = {
  started: false,
  canvasSelector: null,
  worldId: null,
};

async function getWasmModule(onProgress?: EngineLoadListener) {
  if (!loadPromise) {
    loadPromise = (async () => {
      const wasm =
        (await import("@thunderforge/engine/engine")) as BevyWasmModule;

      // Resolved lazily, never at module scope. A static
      // `import ... from "...engine_bg.wasm?url"` makes the dev server
      // handle a ~200MB asset for *any* page that transitively imports this
      // module — including ones that never mount the engine, like the world
      // hub — which measurably slowed unrelated pages. Deferring it here
      // pays that cost only when the engine is actually being mounted.
      const url = await resolveWasmUrl();

      if (onProgress && url) {
        const response = await fetchWasmWithProgress(url, onProgress);
        await wasm.default({ module_or_path: response });
      } else {
        // No listener, or the URL could not be resolved at build time: fall
        // back to letting wasm-bindgen fetch it. Silent but correct, which is
        // the right trade — a missing progress bar must never be a failed
        // load (FR-004's spirit applied to the loader itself).
        await wasm.default();
      }
      return wasm;
    })();

    // Drop the cached promise if it rejects. Without this, `loadPromise`
    // stays pinned to a rejected value and every later call — including
    // FR-032's retry — re-awaits the same failure forever, leaving the retry
    // button permanently decorative. Caught by T051, which is exactly the
    // kind of thing a test asserting only "an error is shown" would miss.
    loadPromise = loadPromise.catch((error: unknown) => {
      loadPromise = null;
      throw error;
    });
  }

  return loadPromise;
}

// Spec 028 (US6, FR-030/FR-031). Spec 008 exposed these same two stages but
// no byte progress, because `wasm.default()` fetches the module internally
// and reports nothing. We now do that fetch ourselves and hand wasm-bindgen
// the `Response`, which keeps streaming instantiation while making the bytes
// observable — see `fetchWasmWithProgress`.
export type EngineLoadStage = "downloading" | "starting";

export interface EngineLoadProgress {
  stage: EngineLoadStage;
  /** Bytes received so far. */
  loaded: number;
  /**
   * Total bytes, when the server said. `null` means genuinely unknown —
   * chunked transfer with no `Content-Length`. FR-030 forbids inventing a
   * percentage in that case, so callers must render an indeterminate state
   * rather than guessing.
   */
  total: number | null;
}

export type EngineLoadListener = (progress: EngineLoadProgress) => void;

/**
 * The engine's wasm URL, or `null` if it cannot be resolved.
 *
 * Dynamic so the asset is never pulled into a page that does not mount the
 * engine. Failure is non-fatal: the caller falls back to letting
 * wasm-bindgen fetch it, losing the progress bar but not the load.
 */
async function resolveWasmUrl(): Promise<string | null> {
  try {
    const mod = (await import("@thunderforge/engine/engine_bg.wasm?url")) as {
      default: string;
    };
    return mod.default ?? null;
  } catch {
    return null;
  }
}

/**
 * Fetch the engine's wasm, reporting bytes as they arrive.
 *
 * Returns a `Response` rather than an `ArrayBuffer` deliberately: passing a
 * Response lets wasm-bindgen keep using `instantiateStreaming`, so
 * compilation overlaps the download instead of waiting for it. Buffering the
 * whole thing to measure it would trade real load time for a progress bar,
 * which is the wrong way round.
 */
async function fetchWasmWithProgress(
  url: string,
  onProgress: EngineLoadListener,
): Promise<Response> {
  // FR-033: no artificial delay for a return visitor. The browser's HTTP
  // cache serves a repeat load from disk, `Content-Length` is still present,
  // and the whole body arrives in one or two chunks — so the loader resolves
  // in a frame or two rather than lingering to be seen. Nothing here waits
  // on a minimum display time, which is the usual way loaders end up
  // *causing* the delay they exist to explain.
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(
      `Engine download failed: ${response.status} ${response.statusText}`,
    );
  }

  const header = response.headers.get("Content-Length");
  // `Content-Length` describes the encoded body, and progress is counted in
  // the same encoded bytes, so the ratio stays honest under gzip/br. Absent
  // header (chunked) means no total is knowable at all.
  const total = header ? Number(header) : null;
  const knownTotal =
    total !== null && Number.isFinite(total) && total > 0 ? total : null;

  if (!response.body) {
    // No streaming available. Report the one honest data point and move on
    // rather than faking intermediate progress.
    onProgress({ stage: "downloading", loaded: 0, total: knownTotal });
    return response;
  }

  let loaded = 0;
  const counted = new ReadableStream<Uint8Array>({
    async start(controller) {
      const reader = response.body!.getReader();
      try {
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          loaded += value.byteLength;
          onProgress({ stage: "downloading", loaded, total: knownTotal });
          controller.enqueue(value);
        }
        controller.close();
      } catch (error) {
        controller.error(error);
      } finally {
        reader.releaseLock();
      }
    },
  });

  // Content-Type must survive, or `instantiateStreaming` refuses the stream.
  return new Response(counted, {
    headers: { "Content-Type": "application/wasm" },
  });
}

export async function mountEngine(
  options: EngineMountOptions,
  onStageChange?: (stage: EngineLoadStage) => void,
  onProgress?: EngineLoadListener,
): Promise<void> {
  onStageChange?.("downloading");
  const module = await getWasmModule(onProgress);
  // FR-031: "starting" is a distinct phase, not the tail of the download.
  // Instantiation of a bundle this size is perceptible, and reporting it as
  // a download stalled at 100% is exactly the "is it broken?" moment this
  // story exists to remove.
  onStageChange?.("starting");
  onProgress?.({ stage: "starting", loaded: 0, total: null });

  state.canvasSelector = options.canvasSelector;
  state.worldId = options.worldId;

  if (!state.started) {
    module.start(options.canvasSelector);
    state.started = true;
  }
}

export async function bindWorldStore(worldStore: WorldStore): Promise<void> {
  const module = await getWasmModule();
  boundWorldStore = worldStore;

  if (!bevyCallbackRegistered && module.set_event_callback) {
    module.set_event_callback((payload: string) => {
      try {
        const command = JSON.parse(payload) as WorldCommand;
        boundWorldStore?.dispatch(command, "bevy");
      } catch {
        // Ignore malformed payloads from the wasm layer.
      }
    });
    bevyCallbackRegistered = true;
  }

  if (worldStoreUnsubscribe) {
    worldStoreUnsubscribe();
    worldStoreUnsubscribe = null;
  }

  worldStoreUnsubscribe = worldStore.subscribe((event) => {
    if (event.source === "bevy") {
      return;
    }

    if (!module.apply_world_command) {
      return;
    }

    const command: WorldCommand = event.command;
    module.apply_world_command(JSON.stringify(command));
  });
}

export function setActiveWorld(worldId: string): void {
  state.worldId = worldId;
}

/**
 * FR-010: tells the engine whether the local session may author
 * walls/shapes (`WallPlugin`/`ShapePlugin` gate all pointer/keyboard
 * authoring input on this). Not modeled as a `WorldCommand` — it's local
 * session state, not something to sync into `WorldState` or broadcast —
 * so this calls the wasm bridge directly rather than going through
 * `worldStore.dispatch`, same shape as `bindWorldStore`'s direct
 * `apply_world_command` calls.
 */
export async function setIsGameMaster(isGameMaster: boolean): Promise<void> {
  const module = await getWasmModule();
  if (!module.apply_world_command) {
    return;
  }
  module.apply_world_command(
    JSON.stringify({ type: "set_is_game_master", isGameMaster }),
  );
}

/**
 * Spec 014 (US4): forwards the per-die detail from an already-resolved
 * `rollDice` response into the engine's `DiceRollPlugin`, purely to
 * animate a reveal — this never asks the engine to decide an outcome
 * (FR-015). Not a `WorldCommand`/`worldStore` event: like
 * `setIsGameMaster`, this is a one-shot local trigger, not persisted
 * world state to sync or broadcast.
 */
export async function triggerDiceRollAnimation(
  dice: { finalValue: number }[],
): Promise<void> {
  const module = await getWasmModule();
  if (!module.apply_world_command) {
    return;
  }
  module.apply_world_command(
    JSON.stringify({ type: "trigger_dice_roll", dice }),
  );
}

/**
 * What one world-open sync did, as reported by the engine.
 *
 * Read-only telemetry. Note what is *not* here: no list of cached items, no
 * fingerprints, nothing that would let this side form a second opinion about
 * what the cache holds. Per research.md R1 and Constitution Principle I,
 * cache policy has exactly one owner and it is the Rust side — TypeScript
 * asks for a sync and reads the outcome, and that is the whole of its
 * involvement.
 */
export interface WorldCacheSyncSummary {
  /** `"synced"` when the plan was applied; `"degraded"` otherwise. */
  status: "synced" | "degraded";
  /** Why it degraded. Absent on success. */
  reason?: string;
  /** Items the client already held and told the server about. */
  held?: number;
  /** Items the server asked for. */
  fetch?: number;
  /** Index rows discarded, and blob files that went with them. */
  evicted?: number;
  blobsRemoved?: number;
  evictFailures?: number;
  /** Assets being pulled ahead of demand, in the background. */
  prefetching?: number;
  canonicalVersion?: number;
}

/**
 * Spec 028 (US1): bring this browser's cache into agreement with the server
 * for the world being opened.
 *
 * One call does everything — derives the OPFS scope from the authenticated
 * user id, sends the manifest, applies the returned plan, and points the
 * engine's asset read path at the result. That is deliberate: the manifest
 * and the fetch/evict decisions live in `thunderforge-cache-browser`, where
 * the index that produces them lives, and never cross this boundary.
 *
 * **Cannot fail.** Resolves to `null` when the engine has no cache entry
 * point at all, and to a `"degraded"` summary for every runtime failure —
 * no OPFS, no session key, an unreachable server, a malformed plan. In every
 * one of those cases the app behaves exactly as it did before this feature
 * existed: assets load from the network. A cache problem must never stop a
 * world from opening, so this swallows rather than rethrows.
 */
export async function syncWorldCache(
  worldId: string,
  userId: string,
): Promise<WorldCacheSyncSummary | null> {
  try {
    const module = await getWasmModule();
    if (!module.sync_world_cache) {
      return null;
    }
    const summary = await module.sync_world_cache(worldId, userId);
    return JSON.parse(summary) as WorldCacheSyncSummary;
  } catch {
    return null;
  }
}

/** One queued change, as the engine hands it over. */
export interface QueuedChangeWire {
  localId: string;
  command: unknown;
}

/**
 * Queue an edit made while disconnected (US7, FR-037).
 *
 * Returns `false` when the change could not be stored — no engine, an older
 * bundle, a failed write. **The caller must not report the edit as accepted in
 * that case.** This is the one path in the cache where a failure has to reach
 * the user, because unlike a missing blob it cannot be recovered by fetching
 * again: the work exists nowhere else.
 */
export async function queueOfflineChange(
  worldId: string,
  localId: string,
  command: unknown,
  isGameMaster: boolean,
): Promise<boolean> {
  try {
    const module = await getWasmModule();
    if (!module.queue_offline_change) return false;
    const result = await module.queue_offline_change(
      worldId,
      localId,
      JSON.stringify(command),
      isGameMaster,
    );
    return (JSON.parse(result) as { queued?: boolean }).queued === true;
  } catch {
    return false;
  }
}

/** Everything queued for a world, in replay order. Empty on any failure. */
export async function readQueuedChanges(worldId: string): Promise<QueuedChangeWire[]> {
  try {
    const module = await getWasmModule();
    if (!module.read_queued_changes) return [];
    return JSON.parse(await module.read_queued_changes(worldId)) as QueuedChangeWire[];
  } catch {
    return [];
  }
}

/**
 * Drop the queued changes the server answered for, keeping the rest.
 *
 * Returns how many are still queued, or `null` if the drain could not run —
 * which is not a failure worth surfacing, because everything simply stays
 * queued and is answered for on the next reconnect.
 */
export async function forgetReconciledChanges(
  outcomes: { localId: string; applied: boolean }[],
): Promise<number | null> {
  try {
    const module = await getWasmModule();
    if (!module.forget_reconciled_changes) return null;
    const result = await module.forget_reconciled_changes(JSON.stringify(outcomes));
    const remaining = (JSON.parse(result) as { remaining?: number }).remaining;
    return typeof remaining === "number" && remaining >= 0 ? remaining : null;
  } catch {
    return null;
  }
}

export function getEngineState(): Readonly<EngineState> {
  return state;
}

export function unmountEngine(): void {
  if (worldStoreUnsubscribe) {
    worldStoreUnsubscribe();
    worldStoreUnsubscribe = null;
  }

  // Bevy owns the render loop once started in wasm.
  // We keep this API for route lifecycle symmetry and future stop/reset hooks.
}
