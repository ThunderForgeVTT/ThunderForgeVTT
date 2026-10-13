import { download, downloadBytes } from "../../services/downloads";
import { setDiceTimingsSource } from "./diceThrow";
import { installEngineProbe } from "./engineProbe";
import { emitEngineLoad } from "./loadSignals";

export type BevyWasmModule = {
  default: (moduleOrPath?: unknown) => Promise<unknown>;
  start: (canvasSelector: string) => void;
  apply_world_command?: (json: string) => void;
  /** Spec 085: the Select bar's Delete, the same request as the Delete key. */
  delete_selection?: () => void;
  set_authoring_mode?: (toolId: string) => boolean;
  set_allowed_authoring_tools?: (toolIds: string) => void;
  clear_allowed_authoring_tools?: () => void;
  set_selection_filter?: (
    tokens: boolean,
    walls: boolean,
    lights: boolean,
    shapes: boolean,
  ) => void;
  begin_token_placement?: (actorId: string) => boolean;
  /**
   * Spec 031 (US3, FR-011). Optional like every other entry point here: a
   * bundle built before props could be carried still mounts, and chrome is
   * told the carry did not begin rather than waiting for a drop that can
   * never come.
   */
  begin_placement?: (kind: string, reference: string) => boolean;
  cancel_token_placement?: () => void;
  authoring_mode?: () => string;
  /**
   * Spec 031 (US4, FR-018). Optional for the same reason as the rest: a
   * bundle built before the scene machine existed must still mount, and its
   * absence means chrome loads a scene the way it always did rather than
   * being an error.
   */
  begin_scene_transition?: (sceneId: string) => boolean;
  complete_scene_transition?: () => void;
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
  /**
   * Spec 028 (US1, T088-T091). Optional like every other cache entry point:
   * a bundle without them means server-only transfer, which is a supported
   * way to run and not an error (FR-048).
   */
  start_peer_transfer?: (
    worldId: string,
    sessionId: string,
    sendSignal: (toSessionId: string, payload: string) => void,
  ) => boolean;
  stop_peer_transfer?: () => void;
  offer_to_peer?: (sessionId: string) => Promise<void>;
  receive_peer_signal?: (
    fromSessionId: string,
    payload: string,
  ) => Promise<void>;
  peer_transfer_activity?: () => string;
  /**
   * Spec 028 (US7, T098/T100/T101). The peer-adjudication protocol, optional
   * like every other cache entry point: a bundle without it means a lost
   * server is plain offline, which is a supported way to run.
   */
  begin_peer_adjudication?: (
    selfUserId: string,
    gmUserId: string,
    onApplied: (changeJson: string) => void,
  ) => boolean;
  peer_adjudication_active?: () => boolean;
  peer_adjudication_server_returned?: () => void;
  end_peer_adjudication?: () => void;
  peer_adjudication_submissions?: () => string;
  propose_token_transform?: (
    entityId: string,
    x?: number,
    y?: number,
    rotation?: number,
    scale?: number,
  ) => boolean;
};

let loadPromise: Promise<BevyWasmModule> | null = null;

export async function getWasmModule(onProgress?: EngineLoadListener) {
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
      installPageDownloader();

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
      setDiceTimingsSource(
        (wasm as { dice_timings?: () => string }).dice_timings ?? null,
      );
      installEngineProbe(wasm);
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
 * The engine's world cache fetches canvas assets through this when it is
 * present (spec 080 FR-022), so they get the same parts, retries and
 * version checks as everything else. Installed before the engine starts.
 */
function installPageDownloader(): void {
  (
    globalThis as {
      __thunderforgeDownloadBytes?: (url: string) => Promise<Uint8Array>;
    }
  ).__thunderforgeDownloadBytes = (url) => downloadBytes(url);
}

/**
 * Fetch the engine's wasm, reporting bytes as they arrive.
 *
 * Returns a `Response` rather than an `ArrayBuffer` deliberately: passing a
 * Response lets wasm-bindgen keep using `instantiateStreaming`, so
 * compilation overlaps the download instead of waiting for it. Buffering the
 * whole thing to measure it would trade real load time for a progress bar,
 * which is the wrong way round.
 *
 * A large uncompressed engine (the development build) arrives in resumable
 * parts, still in order, so streaming compilation is unaffected (spec 080
 * FR-020). A compressed one is one plain request, as before.
 */
async function fetchWasmWithProgress(
  url: string,
  onProgress: EngineLoadListener,
): Promise<Response> {
  // FR-033: no artificial delay for a return visitor. The browser's HTTP
  // cache serves a repeat load from disk and the whole body arrives in one
  // or two chunks — so the loader resolves in a frame or two rather than
  // lingering to be seen. Nothing here waits on a minimum display time,
  // which is the usual way loaders end up *causing* the delay they exist to
  // explain.
  //
  // `total` is `null` when the answer is compressed: its length counts
  // compressed bytes and the body yields decoded ones, so no honest
  // percentage exists (FR-030) and the bar is indeterminate.
  let got;
  try {
    got = await download(url, {
      onProgress: ({ loaded, total }) =>
        onProgress({ stage: "downloading", loaded, total }),
    });
  } catch (error) {
    throw new Error(
      `Engine download failed: ${error instanceof Error ? error.message : String(error)}`,
      { cause: error },
    );
  }
  onProgress({ stage: "downloading", loaded: 0, total: got.total });

  // Content-Type must survive, or `instantiateStreaming` refuses the stream.
  const response = got.toResponse();
  return new Response(response.body?.pipeThrough(countBytes()), {
    headers: { "Content-Type": "application/wasm" },
  });
}

/**
 * Passes the wasm through unchanged and says when its last byte went by
 * (spec 086 US7), so the load's `download` stage ends there and not when
 * the first answer arrived. Streaming compilation is unaffected.
 */
function countBytes(): TransformStream<Uint8Array, Uint8Array> {
  let bytes = 0;
  return new TransformStream({
    transform(chunk, controller) {
      bytes += chunk.byteLength;
      controller.enqueue(chunk);
    },
    flush() {
      emitEngineLoad({ stage: "downloaded", bytes });
    },
  });
}
