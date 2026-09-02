import {
  asSdkError,
  SDK_VERSION,
  type EngineSdkError,
} from "@/engine/sdk/commands";
import { createClient, type Client } from "graphql-ws";

import { postGraphQL } from "../../api/graphqlClient";
import {
  isPeerTransferEnabled,
  reportPeerTransferActivity,
} from "../../services/peerTransfer";
import type { EngineMountOptions, EngineState } from "./types";
import type { WorldStore } from "../world/store";
import type { WorldCommand } from "../world/types";

type BevyWasmModule = {
  default: (moduleOrPath?: unknown) => Promise<unknown>;
  start: (canvasSelector: string) => void;
  apply_world_command?: (json: string) => void;
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

/**
 * Everyone listening for commands the engine refused.
 *
 * A refusal that reaches nobody is indistinguishable from the silent drop the
 * SDK version stamp exists to retire, so the report needs somewhere to land.
 * Listeners are notified; they cannot inject, which keeps this an observation
 * surface rather than a way to fake engine failures in a test.
 */
const sdkErrorListeners = new Set<(error: EngineSdkError) => void>();

/** Subscribe to SDK errors. Returns the unsubscribe. */
export function onSdkError(
  listener: (error: EngineSdkError) => void,
): () => void {
  sdkErrorListeners.add(listener);
  return () => {
    sdkErrorListeners.delete(listener);
  };
}

function reportSdkError(error: EngineSdkError): void {
  // Logged as well as dispatched: a bundle drift that nobody has subscribed
  // to still needs to be visible to whoever is looking at the console.
  console.error(
    `[engine sdk] ${error.code}: ${error.message}`,
    error.command ?? "",
  );
  for (const listener of sdkErrorListeners) {
    try {
      listener(error);
    } catch {
      // One bad listener must not stop the others hearing about this.
    }
  }
}

/**
 * The engine asking for a lore entry to be opened.
 *
 * Spec 030, US1. The engine recognises the effect and resolves which entry it
 * points at; opening a tab needs the application's URL structure, which is
 * chrome and belongs on this side (Constitution Principle I).
 */
export interface OpenLoreEvent {
  type: "openLore";
  interactiveId: string;
  entryId: string;
}

function asOpenLore(event: unknown): OpenLoreEvent | null {
  if (typeof event !== "object" || event === null) return null;
  const candidate = event as { type?: unknown };
  return candidate.type === "openLore" ? (event as OpenLoreEvent) : null;
}

/** A player asked to pick a placed item up. Chrome asks the server. */
export interface PickUpItemEvent {
  type: "pickUpItem";
  interactiveId: string;
  itemId: string;
  /** The scene token to remove once the server agrees. */
  subjectRef?: string;
}

function asPickUpItem(event: unknown): PickUpItemEvent | null {
  const candidate = event as { type?: unknown };
  return candidate.type === "pickUpItem" ? (event as PickUpItemEvent) : null;
}

/**
 * An interactive whose effect this build cannot perform.
 *
 * ADR-054: absence is detected *before* dispatch, by comparing the stored
 * effect id against the assembled registry — never by noticing that a
 * fire-and-forget dispatch did nothing. A Game Master whose build lacks the
 * subsystem has a switch they cannot use, not a scene that failed to load.
 */
export interface InteractionUnavailableEvent {
  type: "interactionUnavailable";
  interactiveId: string;
  effectId: string;
}

function asInteractionUnavailable(
  event: unknown,
): InteractionUnavailableEvent | null {
  const candidate = event as { type?: unknown };
  return candidate.type === "interactionUnavailable"
    ? (event as InteractionUnavailableEvent)
    : null;
}

/**
 * A tool was taken away while this person was holding it.
 *
 * The engine has already dropped them back to Select and discarded whatever
 * gesture was in flight — that part is not chrome's decision. What chrome owes
 * is legibility: spec 031's edge case is explicit that the tool must not
 * "silently cease to respond", which is precisely what a mid-gesture
 * revocation looks like without this.
 */
export interface AuthoringToolRevokedEvent {
  type: "authoringToolRevoked";
  /** The tool that was being used when it was lost. */
  tool: string;
}

function asAuthoringToolRevoked(
  event: unknown,
): AuthoringToolRevokedEvent | null {
  const candidate = event as { type?: unknown };
  return candidate.type === "authoringToolRevoked"
    ? (event as AuthoringToolRevokedEvent)
    : null;
}

const authoringToolRevokedListeners = new Set<
  (event: AuthoringToolRevokedEvent) => void
>();

/** Be told when a tool was revoked mid-use. Returns the unsubscribe. */
export function onAuthoringToolRevoked(
  listener: (event: AuthoringToolRevokedEvent) => void,
): () => void {
  authoringToolRevokedListeners.add(listener);
  return () => {
    authoringToolRevokedListeners.delete(listener);
  };
}

/**
 * A carry that ended, one way or the other.
 *
 * `kind` is the word chrome used when it asked for the carry — `actor` for a
 * token from the actors pane, `prop` for something authored in the
 * interactions panel — and `reference` is whatever it handed over with it.
 * The engine interprets neither: it reports where the drop happened and this
 * side decides what, if anything, comes to exist there.
 */
export interface PlacementConfirmedEvent {
  type: "token_placement_confirmed";
  kind: string;
  reference: string;
  x: number;
  y: number;
}

function asPlacementConfirmed(event: unknown): PlacementConfirmedEvent | null {
  const candidate = event as { type?: unknown };
  return candidate.type === "token_placement_confirmed"
    ? (event as PlacementConfirmedEvent)
    : null;
}

const placementConfirmedListeners = new Set<
  (event: PlacementConfirmedEvent) => void
>();
const placementCancelledListeners = new Set<() => void>();

/**
 * Be told where a carried thing was dropped.
 *
 * Nothing exists yet when this fires. The engine has reported a position and
 * gone back to `Idle`; whether a token is created is the server's decision,
 * asked for from here (Constitution Principle I).
 */
export function onPlacementConfirmed(
  listener: (event: PlacementConfirmedEvent) => void,
): () => void {
  placementConfirmedListeners.add(listener);
  return () => {
    placementConfirmedListeners.delete(listener);
  };
}

/**
 * Be told that a carry ended with nothing placed.
 *
 * Subscribed to as well as the confirmation because a panel that only hears
 * about drops would go on claiming it is placing something after Escape, a
 * tool change or a scene change abandoned the carry — and the engine treats
 * all four identically, which is the point of its one `OnExit`.
 */
export function onPlacementCancelled(listener: () => void): () => void {
  placementCancelledListeners.add(listener);
  return () => {
    placementCancelledListeners.delete(listener);
  };
}

const pickUpItemListeners = new Set<(event: PickUpItemEvent) => void>();
const unavailableListeners = new Set<
  (event: InteractionUnavailableEvent) => void
>();

/** Subscribe to pickup requests. Returns the unsubscribe. */
export function onPickUpItem(
  listener: (event: PickUpItemEvent) => void,
): () => void {
  pickUpItemListeners.add(listener);
  return () => {
    pickUpItemListeners.delete(listener);
  };
}

/** Subscribe to unavailable-interaction reports. Returns the unsubscribe. */
export function onInteractionUnavailable(
  listener: (event: InteractionUnavailableEvent) => void,
): () => void {
  unavailableListeners.add(listener);
  return () => {
    unavailableListeners.delete(listener);
  };
}

const openLoreListeners = new Set<(event: OpenLoreEvent) => void>();

/**
 * Be told when an interactive wants a lore page opened.
 *
 * A listener rather than a direct `window.open` here, because a popup blocker
 * treats a call made outside a user gesture very differently from one made
 * inside it — and only the component that handled the click knows which it is.
 */
export function onOpenLore(
  listener: (event: OpenLoreEvent) => void,
): () => void {
  openLoreListeners.add(listener);
  return () => {
    openLoreListeners.delete(listener);
  };
}

function reportOpenLore(event: OpenLoreEvent): void {
  for (const listener of openLoreListeners) {
    try {
      listener(event);
    } catch {
      // One bad listener must not stop the others hearing about this.
    }
  }
}

/**
 * The engine reporting that something was triggered without being clicked.
 *
 * Spec 030, US5. A token crossed into a region. The engine detects it because
 * it is the only thing that knows both where the token was and where it is
 * now — and it *reports* rather than performs, because whether the crossing is
 * permitted is the server's decision (Principle III, ADR-054).
 */
export interface InteractionTriggeredEvent {
  type: "interactionTriggered";
  interactiveId: string;
  trigger: "enter";
}

function asInteractionTriggered(
  event: unknown,
): InteractionTriggeredEvent | null {
  if (typeof event !== "object" || event === null) return null;
  const candidate = event as { type?: unknown };
  return candidate.type === "interactionTriggered"
    ? (event as InteractionTriggeredEvent)
    : null;
}

const triggerListeners = new Set<(event: InteractionTriggeredEvent) => void>();

/** Be told when the engine detected a trigger that nobody clicked. */
export function onInteractionTriggered(
  listener: (event: InteractionTriggeredEvent) => void,
): () => void {
  triggerListeners.add(listener);
  return () => {
    triggerListeners.delete(listener);
  };
}

function reportInteractionTriggered(event: InteractionTriggeredEvent): void {
  for (const listener of triggerListeners) {
    try {
      listener(event);
    } catch {
      // One bad listener must not stop the others hearing about this.
    }
  }
}

/**
 * The engine asking for a scene's contents.
 *
 * Emitted by the scene-transition machine at the one moment loading is safe:
 * the previous scene's tokens, walls, lights and shapes are already off the
 * canvas, and the machine has settled into `Loading` to wait. Fetching
 * alongside `beginSceneTransition` instead would race the engine's despawn,
 * which runs on its own frame — and the frame usually wins, which is the
 * worst of the two outcomes because the times it loses look like a map that
 * randomly came up empty.
 */
export interface SceneLoadRequestedEvent {
  type: "scene_load_requested";
  sceneId: string;
}

function asSceneLoadRequested(event: unknown): SceneLoadRequestedEvent | null {
  if (typeof event !== "object" || event === null) return null;
  const candidate = event as { type?: unknown };
  return candidate.type === "scene_load_requested"
    ? (event as SceneLoadRequestedEvent)
    : null;
}

const sceneLoadRequestedListeners = new Set<
  (event: SceneLoadRequestedEvent) => void
>();

/**
 * Be told when the engine has cleared the canvas and wants a scene's content.
 *
 * A listener rather than a promise from `beginSceneTransition`: the machine
 * can be sent somewhere else mid-transition, so "which scene was asked for"
 * has to travel with the event rather than be assumed from the call that
 * started it.
 */
export function onSceneLoadRequested(
  listener: (event: SceneLoadRequestedEvent) => void,
): () => void {
  sceneLoadRequestedListeners.add(listener);
  return () => {
    sceneLoadRequestedListeners.delete(listener);
  };
}

function reportSceneLoadRequested(event: SceneLoadRequestedEvent): void {
  for (const listener of sceneLoadRequestedListeners) {
    try {
      listener(event);
    } catch {
      // One bad listener must not stop the others hearing about this.
    }
  }
}

export async function bindWorldStore(worldStore: WorldStore): Promise<void> {
  const module = await getWasmModule();
  boundWorldStore = worldStore;

  if (!bevyCallbackRegistered && module.set_event_callback) {
    module.set_event_callback((payload: string) => {
      try {
        const parsed: unknown = JSON.parse(payload);

        // An SDK error is the engine reporting a command it refused. It is
        // emitted on the same channel as world events but it is not one:
        // dispatching it would put a command the store has never heard of
        // into world state, which is worse than the silent drop this
        // reporting exists to retire.
        const sdkError = asSdkError(parsed);
        if (sdkError) {
          reportSdkError(sdkError);
          return;
        }

        // Nor is a lore-open request a world command. It asks the
        // application to do something, and dispatching it would put a
        // command nothing reduces into the store.
        const openLore = asOpenLore(parsed);
        if (openLore) {
          reportOpenLore(openLore);
          return;
        }

        // Nor is a detected trigger. It asks the application to go and ask
        // the server, and it is not a change to world state.
        const triggered = asInteractionTriggered(parsed);
        if (triggered) {
          reportInteractionTriggered(triggered);
          return;
        }

        // Nor is a pickup request: it asks the server for a change, and the
        // engine has removed nothing. Dispatching it would put a command
        // nothing reduces into the store — the same reason lore-open and
        // trigger detection are routed out above.
        const pickUp = asPickUpItem(parsed);
        if (pickUp) {
          for (const listener of pickUpItemListeners) {
            listener(pickUp);
          }
          return;
        }

        // Nor is a report that this build cannot perform an effect. It is
        // something to tell the Game Master, not a change to the world.
        const unavailable = asInteractionUnavailable(parsed);
        if (unavailable) {
          for (const listener of unavailableListeners) {
            listener(unavailable);
          }
          return;
        }

        // Nor is either end of a placement. Both report a gesture, and the
        // one that matters carries no world change at all: nothing has been
        // created when a drop is confirmed, which is precisely why chrome
        // hears about it. Dispatching either would put a command nothing
        // reduces into the store.
        const placed = asPlacementConfirmed(parsed);
        if (placed) {
          for (const listener of placementConfirmedListeners) {
            listener(placed);
          }
          return;
        }
        if (
          (parsed as { type?: unknown }).type === "token_placement_cancelled"
        ) {
          for (const listener of placementCancelledListeners) {
            listener();
          }
          return;
        }

        // Nor is a revoked tool. The engine has already left the mode; this
        // says so, so the rail can stop claiming the tool is armed and the
        // person is told why the canvas stopped answering.
        const revoked = asAuthoringToolRevoked(parsed);
        if (revoked) {
          for (const listener of authoringToolRevokedListeners) {
            listener(revoked);
          }
          return;
        }

        // Nor is the scene machine's request for content. It asks chrome to
        // go and fetch — the engine owns no network by design — and there is
        // no store command by that name for it to become.
        const sceneLoad = asSceneLoadRequested(parsed);
        if (sceneLoad) {
          reportSceneLoadRequested(sceneLoad);
          return;
        }

        // The machine's other two notices are dropped here rather than left
        // to fall through. They report where a transition got to, nothing in
        // world state changes because of them, and dispatching them would put
        // commands nothing reduces into the store — the same reason every
        // report above is routed out. Chrome learns a transition finished
        // from the state it already tracks, not from these.
        const sceneNotice = (parsed as { type?: unknown }).type;
        if (
          sceneNotice === "scene_unloaded" ||
          sceneNotice === "scene_transition_complete"
        ) {
          return;
        }

        boundWorldStore?.dispatch(parsed as WorldCommand, "bevy");
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

    // Stamped centrally rather than at each call site. A version remembered
    // on most commands and forgotten on one leaves exactly one path failing
    // silently, which is the hardest kind of gap to find — and the engine
    // treats an absent version as "no claim" rather than as agreement, so a
    // missed stamp would not even be reported.
    const command: WorldCommand = event.command;
    module.apply_world_command(
      JSON.stringify({ ...command, sdkVersion: SDK_VERSION }),
    );
  });
}

/**
 * The world store this bridge is bound to, or `null`.
 *
 * Exists so a caller outside the React tree — chiefly an end-to-end test
 * driving the same path the application does — can dispatch through the store
 * the engine is actually listening to, rather than constructing a second one
 * that nothing is wired to.
 *
 * Read-only. It hands back the store, which is already the application's own
 * mutation surface; it does not add a way to reach the engine that the
 * application itself lacks.
 */
/**
 * Tell the engine which authoring tool is armed.
 *
 * The engine is the authority for this (Constitution I): every canvas input
 * system is engine-side, and until now none of them knew which tool the Game
 * Master had chosen — `openGmToolId` lived in React and only picked a flyout.
 * That is why a click could be claimed by wall drawing while the rail showed
 * Lights.
 *
 * Optional by design. A bundle that predates the mode simply does not export
 * it, and the caller carries on: the tool rail still renders, and the engine
 * behaves as it did before. Never throws — a failed mode change must not take
 * the canvas down with it.
 *
 * Returns whether the engine recognised the tool.
 */
export async function setAuthoringMode(toolId: string): Promise<boolean> {
  try {
    const module = await getWasmModule();
    if (!module.set_authoring_mode) return false;
    return module.set_authoring_mode(toolId);
  } catch {
    return false;
  }
}

/**
 * Tell the engine which tools this viewer may use.
 *
 * Not a hint. The engine refuses a mode request outside this set and disarms
 * the input systems for anything outside it, so a client that skips this call
 * gets the unrestricted default — which is why the caller must send `""` for a
 * person with no tools rather than simply not calling (FR-045: the default the
 * *server* declares is Game-Master-only, and an engine told nothing cannot
 * know that).
 *
 * Optional by design, like `setAuthoringMode`: a bundle predating tool
 * permissions does not export it, and the role checks that gated authoring
 * before still apply.
 */
export async function setAllowedAuthoringTools(
  toolIds: readonly string[],
): Promise<void> {
  try {
    const module = await getWasmModule();
    module.set_allowed_authoring_tools?.(toolIds.join(","));
  } catch {
    // A failed restriction must not take the canvas down. The server refused
    // the write regardless; this call only decides what is offered.
  }
}

/**
 * Remove any tool restriction from the engine.
 *
 * For a viewer who holds every tool. Distinct from sending the full list so
 * the engine's "no declaration" default and "granted everything" state stay
 * distinguishable when reading it back.
 */
export async function clearAllowedAuthoringTools(): Promise<void> {
  try {
    const module = await getWasmModule();
    module.clear_allowed_authoring_tools?.();
  } catch {
    // As above.
  }
}

/**
 * Which tool the engine currently has armed, or `null` if it cannot say.
 *
 * Read-only, and here so the boundary can be observed directly rather than
 * inferred from what a click did.
 */
export async function getAuthoringMode(): Promise<string | null> {
  try {
    const module = await getWasmModule();
    if (!module.authoring_mode) return null;
    return module.authoring_mode();
  } catch {
    return null;
  }
}

/**
 * Tell the engine which kinds the Select tool acts on.
 *
 * Selection is engine state; this only carries the person's preference across
 * the boundary. Optional like the rest — a bundle without it leaves selection
 * unfiltered, which is the pre-existing behaviour and a safe place to land.
 */
export async function setSelectionFilter(
  tokens: boolean,
  walls: boolean,
  lights: boolean,
  shapes: boolean,
): Promise<void> {
  try {
    const module = await getWasmModule();
    module.set_selection_filter?.(tokens, walls, lights, shapes);
  } catch {
    // A filter that cannot be applied is a tool that selects everything, which
    // is what it did before this existed.
  }
}

/**
 * Attach an actor's token to the cursor, to be placed by a left click.
 *
 * The engine owns the carry: the preview, the snapping and the cancel all live
 * there, because screen-to-world is the camera's business and a React element
 * following the mouse would drift against it.
 *
 * Returns whether the carry began.
 */
export async function beginTokenPlacement(actorId: string): Promise<boolean> {
  try {
    const module = await getWasmModule();
    return module.begin_token_placement?.(actorId) ?? false;
  } catch {
    return false;
  }
}

/**
 * Carry something that is not an actor's token, to be placed by a left click.
 *
 * The same gesture as `beginTokenPlacement`, and deliberately the same machine
 * (`src/engine/src/plugins/placement.rs`): a second one would have to re-derive
 * snapping, the preview, and abandoning the carry when the tool or the scene
 * changes. `reference` is opaque to the engine and comes back on the drop.
 *
 * Returns whether the carry began. `false` means this bundle predates
 * `begin_placement`, so no drop will ever be reported and a caller waiting for
 * one would wait forever.
 */
export async function beginPropPlacement(reference = ""): Promise<boolean> {
  try {
    const module = await getWasmModule();
    return module.begin_placement?.("prop", reference) ?? false;
  } catch {
    return false;
  }
}

/** Abandon a placement in progress. Leaves nothing behind. */
export async function cancelTokenPlacement(): Promise<void> {
  try {
    const module = await getWasmModule();
    module.cancel_token_placement?.();
  } catch {
    // Nothing to abandon.
  }
}

/**
 * Begin swapping the canvas over to `sceneId`.
 *
 * The engine takes the previous scene's tokens, walls, lights, shapes,
 * interactives and selection off the canvas itself (FR-018) and then emits
 * `scene_load_requested`. Subscribe with `onSceneLoadRequested` and fetch
 * *then*: the despawn happens on a later frame, so anything fetched from here
 * is racing it.
 *
 * Returns whether the engine took the request. `false` means the machine is
 * absent from this bundle, the wasm layer is unreachable, or the id was
 * empty — in every one of those cases nothing will be despawned and no
 * request will ever be emitted, so a caller waiting for one would wait
 * forever and must load by itself instead.
 */
export async function beginSceneTransition(sceneId: string): Promise<boolean> {
  try {
    const module = await getWasmModule();
    return module.begin_scene_transition?.(sceneId) ?? false;
  } catch {
    return false;
  }
}

/**
 * Tell the engine that every command for the new scene has been sent.
 *
 * This is not optional politeness. `Loading` has no timeout on purpose — the
 * engine has no way to judge whether chrome has failed or is merely slow — so
 * a load path that reaches its end without calling this leaves the machine
 * waiting for the rest of a scene that is already fully here. Call it on
 * every exit from the load, the failed ones included: "chrome has stopped
 * sending" is the honest report, and a partly-loaded scene the person can see
 * is better than a transition that never ends.
 */
export async function completeSceneTransition(): Promise<void> {
  try {
    const module = await getWasmModule();
    module.complete_scene_transition?.();
  } catch {
    // A bundle without the machine was never in a transition to finish.
  }
}

export function getBoundWorldStore(): WorldStore | null {
  return boundWorldStore;
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
 * Spec 032: hands the world's interface pack its canvas half.
 *
 * The engine has accepted `set_display_appearance` since spec 029 — the
 * command parses, `StatusDisplayPlugin` owns the `Appearance` resource it
 * writes, and the TypeScript SDK types it in `sdk/commands.ts`. Until now
 * nothing called it. Every layer existed and nothing joined them, which is the
 * same shape as the two dead paths found during spec 031.
 *
 * Like `setIsGameMaster`, this goes to the wasm bridge directly rather than
 * through `worldStore.dispatch`: an appearance is presentation, not world
 * state to persist or broadcast. The server already told everyone the pack
 * changed; this is each client applying it to its own canvas.
 *
 * A pack with no `canvas` block sends nothing, and the engine keeps its own
 * defaults — an empty override would say "reset everything to nothing", which
 * is a different claim.
 */
export async function setDisplayAppearance(
  appearance: Record<string, unknown> | null,
): Promise<void> {
  if (!appearance) {
    return;
  }
  const module = await getWasmModule();
  if (!module.apply_world_command) {
    return;
  }
  module.apply_world_command(
    JSON.stringify({ type: "set_display_appearance", appearance }),
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
  /**
   * The FR-019 integrity pass, reported with the plan it rides with: index
   * rows dropped because their blob was gone or unreadable, and orphaned
   * blobs reclaimed. Surfaced because FR-051 asks the diagnostics view for
   * "any integrity repairs performed" — a store that silently repaired
   * itself is exactly the thing a user should be able to see happened.
   */
  rowsRepaired?: number;
  blobsReclaimed?: number;
  repairFailures?: number;
  /**
   * FR-024: even releasing everything permissible left no room, so the cache
   * is serving what it holds and has stopped adding. Not a failure, but the
   * one cache state that explains an otherwise inexplicable readout — a
   * warm-looking world that keeps fetching.
   */
  budgetInsufficient?: boolean;
  /** Assets being pulled ahead of demand, in the background. */
  prefetching?: number;
  canonicalVersion?: number;
  /**
   * Whether anyone else was live in this world at sync time (spec 028 T086).
   * Reachability, not holdings — it never means a peer has any given bytes,
   * and a `false` never suppresses a server fetch.
   */
  peerAvailable?: boolean;
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
    // Before the sync, not after, and this is the only ordering that works:
    // `sync_world_cache` is what hands the peer module its entitlement scope
    // (`plan.fetch`) and its servable set, and it can only hand them to a
    // module that is already running. Started after, the client would spend a
    // whole world open unable to ask any peer for anything.
    //
    // Awaited, but cheap — it resolves once the engine can talk to peers, and
    // leaves the roster round trip running behind it. Failure is invisible on
    // purpose: server-only transfer is a supported way to run (SC-013).
    await startPeerTransfer(worldId);

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
export async function readQueuedChanges(
  worldId: string,
): Promise<QueuedChangeWire[]> {
  try {
    const module = await getWasmModule();
    if (!module.read_queued_changes) return [];
    return JSON.parse(
      await module.read_queued_changes(worldId),
    ) as QueuedChangeWire[];
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
    const result = await module.forget_reconciled_changes(
      JSON.stringify(outcomes),
    );
    const remaining = (JSON.parse(result) as { remaining?: number }).remaining;
    return typeof remaining === "number" && remaining >= 0 ? remaining : null;
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Peer-assisted content distribution (spec 028 T088-T091, FR-044 to FR-050)
// ---------------------------------------------------------------------------

/**
 * The signaling half, and only the signaling half.
 *
 * The protocol — framing, what may be requested, verification, what may be
 * served, rate limits — lives in `crates/thunderforge-cache-browser/src/peer.rs`
 * and never crosses this boundary. What is here is the part TypeScript already
 * owns: the `graphql-ws` connection the SDP and ICE payloads ride (ADR-048),
 * and the user's setting. The engine hands out opaque strings and takes opaque
 * strings back; nothing in this file interprets one.
 */
const PEER_SIGNAL_MUTATION = `
mutation SendPeerSignal($input: PeerSignalInput!) {
  sendPeerSignal(input: $input)
}`;

const PEER_SESSIONS_QUERY = `
query PeerSessions($worldId: UUID!) {
  peerSessions(worldId: $worldId)
}`;

const PEER_SIGNALS_SUBSCRIPTION = `
subscription PeerSignals($worldId: UUID!, $sessionId: String!) {
  peerSignals(worldId: $worldId, sessionId: $sessionId) {
    fromSessionId
    payload
  }
}`;

/** How often the indicator is refreshed while peer transfer is running. */
const PEER_ACTIVITY_POLL_MS = 1_000;

interface PeerTransferSession {
  worldId: string;
  sessionId: string;
  client: Client;
  dispose: () => void;
  poll: ReturnType<typeof setInterval>;
}

let peerSession: PeerTransferSession | null = null;

/**
 * Start peer transfer for one world.
 *
 * **The setting is consulted first, before anything opens.** That ordering is
 * the whole point of the check: a direct peer connection is what reveals an IP
 * address to another participant, so "disabled" has to mean no connection was
 * ever attempted, not that one was attempted and its bytes ignored
 * (`services/peerTransfer.ts`).
 *
 * Never throws and never reports failure. Every way this can not happen — the
 * setting is off, the bundle has no peer entry points, the signaling server
 * does not answer, no other session is live — leaves the client fetching from
 * the server, which is what it would have done anyway (FR-048, SC-013).
 */
export async function startPeerTransfer(worldId: string): Promise<boolean> {
  if (!isPeerTransferEnabled()) return false;
  if (peerSession?.worldId === worldId) return true;
  stopPeerTransfer();

  try {
    const module = await getWasmModule();
    if (
      !module.start_peer_transfer ||
      !module.receive_peer_signal ||
      !module.offer_to_peer
    ) {
      return false;
    }

    // One session id per page load, generated here and meaningful to nobody
    // else. It is an address for signals and not an identity: it is not the
    // user id, it is not stored, and it does not survive a reload (FR-050).
    const sessionId = crypto.randomUUID();

    const started = module.start_peer_transfer(
      worldId,
      sessionId,
      (to, payload) => {
        // Fire and forget. A signal that does not arrive costs one peer
        // connection, and the contract already says the server does not
        // promise reachability.
        // `fromSessionId` is mandatory and **verified, not trusted**: the
        // server only relays as a session that is registered, in this world,
        // to this user. So it must be the same id the subscription registered
        // with, and a mismatch is silently `false` rather than an error.
        //
        // A `false` reply is "that peer is gone" — the session ended, or lost
        // membership — and is not a transport failure. There is nothing to
        // retry: an unanswered offer simply never becomes a channel, and the
        // fetch it would have served falls to the server like every other
        // peer failure.
        void postGraphQL(PEER_SIGNAL_MUTATION, {
          input: {
            worldId,
            fromSessionId: sessionId,
            toSessionId: to,
            payload,
          },
        }).catch(() => undefined);
      },
    );
    if (!started) return false;

    // A connection of this module's own rather than the world-event socket's
    // singleton, which `engine/world/sync/subscriptionClient.ts` keeps private.
    // It is opened only for a world page with peer transfer enabled and closed
    // with it, so the cost is one socket for as long as the feature is on —
    // and sharing the other one would mean reaching into a module this change
    // does not own.
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const client = createClient({
      url: `${protocol}//${window.location.host}/api/ws`,
      retryAttempts: Infinity,
    });

    const dispose = client.subscribe<{
      peerSignals: { fromSessionId: string; payload: string };
    }>(
      { query: PEER_SIGNALS_SUBSCRIPTION, variables: { worldId, sessionId } },
      {
        next: (result) => {
          const signal = result.data?.peerSignals;
          if (!signal) return;
          void module.receive_peer_signal?.(
            signal.fromSessionId,
            signal.payload,
          );
        },
        // Signaling unavailable means peer transfer is off for the session and
        // everything else works (peer-protocol.md, "Failure modes"). There is
        // nothing to tell the user and nothing for them to do.
        error: () => undefined,
        complete: () => undefined,
      },
    );

    const poll = setInterval(() => {
      try {
        const raw = module.peer_transfer_activity?.();
        if (raw) {
          reportPeerTransferActivity(
            JSON.parse(raw) as {
              connectedPeers: number;
              bytesFromPeers: number;
              verificationFailures: number;
            },
          );
        }
      } catch {
        // A poll that fails is one missed indicator refresh.
      }
    }, PEER_ACTIVITY_POLL_MS);

    peerSession = { worldId, sessionId, client, dispose, poll };

    // The newcomer always initiates: query the roster and offer to each. That
    // is why there is no join/leave push in the contract — arrivals offer, and
    // departures are noticed when a channel closes.
    //
    // Deliberately not awaited. This function resolves once the engine is
    // *able* to talk to peers, and the caller's next act is the world-cache
    // sync that decides what may be asked for; making it wait on a roster
    // round trip would put a network hop in front of the thing the user is
    // actually watching, for a benefit that lands seconds later anyway.
    void (async () => {
      try {
        const roster = await postGraphQL<{ peerSessions: string[] }>(
          PEER_SESSIONS_QUERY,
          { worldId },
        );
        // The roster already excludes every session belonging to this user,
        // not merely this one: two tabs of one browser share an origin and
        // therefore share the cache a transfer would be filling, so there is
        // nothing to gain between them.
        for (const peer of roster.peerSessions ?? []) {
          // Stop if peer transfer was torn down while the roster was in
          // flight; offering from a session that no longer exists would be
          // relayed as nobody.
          if (peerSession?.sessionId !== sessionId) return;
          void module.offer_to_peer?.(peer);
        }
      } catch {
        // No roster is "no peers available": server fetch, no user-visible
        // difference. The subscription stays open, so a later arrival
        // offering to us still connects.
      }
    })();

    return true;
  } catch {
    stopPeerTransfer();
    return false;
  }
}

// ---------------------------------------------------------------------------
// Peer adjudication (spec 028 US7, T098/T100/T101, FR-057 to FR-062)
// ---------------------------------------------------------------------------

/**
 * Ask the peer fabric to begin adjudicated play.
 *
 * **The caller decides the server is gone**, from the heartbeat and nothing
 * else — this is signalling, not a second opinion about connectivity. What
 * the fabric decides is the rest: whether there is a table to play with at
 * all, and, on a player's client, whether the Game Master is among them,
 * which is a thing no count on this side can answer (FR-059).
 *
 * Never throws. Every way it can fail — no bundle, no peers, peer transfer
 * off — is plain offline, which is a path that already works.
 */
export async function beginPeerAdjudication(
  selfUserId: string,
  gmUserId: string,
  onApplied: (changeJson: string) => void,
): Promise<boolean> {
  try {
    const module = await getWasmModule();
    return (
      module.begin_peer_adjudication?.(selfUserId, gmUserId, onApplied) ?? false
    );
  } catch {
    return false;
  }
}

/** Whether peer-adjudicated play is running right now. */
export async function peerAdjudicationActive(): Promise<boolean> {
  try {
    const module = await getWasmModule();
    return module.peer_adjudication_active?.() ?? false;
  } catch {
    return false;
  }
}

/**
 * The server is reachable again: stop adjudicating, and keep what was applied
 * for submission (FR-062).
 */
export async function endPeerAdjudication(
  serverReturned: boolean,
): Promise<void> {
  try {
    const module = await getWasmModule();
    if (serverReturned) {
      module.peer_adjudication_server_returned?.();
    } else {
      module.end_peer_adjudication?.();
    }
  } catch {
    // Nothing to stop, or no bundle to stop it in.
  }
}

/**
 * Everything applied while server-isolated, as JSON.
 *
 * Provisional, all of it. The Game Master's client submits these over its own
 * authenticated session and the server confirms or rejects each one; its
 * decision is final (FR-062).
 */
export async function peerAdjudicationSubmissions(): Promise<string> {
  try {
    const module = await getWasmModule();
    return module.peer_adjudication_submissions?.() ?? "[]";
  } catch {
    return "[]";
  }
}

/**
 * Put one token movement to the table.
 *
 * Position, rotation and scale only, and there is no argument for anything
 * else (FR-060). `false` means "not adjudicating — queue it in the outbox
 * instead", which is the caller's single fall-back.
 */
export async function proposeTokenTransform(
  entityId: string,
  transform: { x?: number; y?: number; rotation?: number; scale?: number },
): Promise<boolean> {
  try {
    const module = await getWasmModule();
    return (
      module.propose_token_transform?.(
        entityId,
        transform.x,
        transform.y,
        transform.rotation,
        transform.scale,
      ) ?? false
    );
  } catch {
    return false;
  }
}

/**
 * Stop peer transfer and close every channel.
 *
 * Called when the world closes, when the user turns the setting off, and on
 * unload. Idempotent, and safe to call when it was never started — peer
 * connections must not outlive the session that justified them (FR-050).
 */
export function stopPeerTransfer(): void {
  const session = peerSession;
  peerSession = null;
  if (session) {
    clearInterval(session.poll);
    try {
      session.dispose();
    } catch {
      // Already gone.
    }
    void session.client.dispose();
  }
  reportPeerTransferActivity({
    connectedPeers: 0,
    bytesFromPeers: 0,
    verificationFailures: 0,
  });
  void getWasmModule()
    .then((module) => module.stop_peer_transfer?.())
    .catch(() => undefined);
}

/** Whether peer transfer is running, for tests and diagnostics. */
export function isPeerTransferRunning(): boolean {
  return peerSession !== null;
}

export function getEngineState(): Readonly<EngineState> {
  return state;
}

export function unmountEngine(): void {
  // FR-050: peer connections do not outlive the world session they belong to.
  stopPeerTransfer();

  if (worldStoreUnsubscribe) {
    worldStoreUnsubscribe();
    worldStoreUnsubscribe = null;
  }

  // Bevy owns the render loop once started in wasm.
  // We keep this API for route lifecycle symmetry and future stop/reset hooks.
}
