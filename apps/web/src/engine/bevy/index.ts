import type { EngineMountOptions, EngineState } from "./types";
import type { WorldStore } from "../world/store";
import type { WorldCommand } from "../world/types";

type BevyWasmModule = {
  default: (moduleOrPath?: unknown) => Promise<unknown>;
  start: (canvasSelector: string) => void;
  apply_world_command?: (json: string) => void;
  set_event_callback?: (callback: (json: string) => void) => void;
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

async function getWasmModule() {
  if (!loadPromise) {
    loadPromise = (async () => {
      const wasm = (await import(
        "@thunderforge/engine/engine"
      )) as BevyWasmModule;
      await wasm.default();
      return wasm;
    })();
  }

  return loadPromise;
}

// Spec 008 (US1, FR-002): the only two real phase boundaries this loader
// exposes — "downloading" spans both the dynamic import and wasm-bindgen's
// own init/instantiation (getWasmModule's whole body), "starting" is the
// final `module.start()` call. No byte-level progress is available through
// the APIs in use here (research.md §3) — status text, not a percentage.
export type EngineLoadStage = "downloading" | "starting";

export async function mountEngine(
  options: EngineMountOptions,
  onStageChange?: (stage: EngineLoadStage) => void,
): Promise<void> {
  onStageChange?.("downloading");
  const module = await getWasmModule();
  onStageChange?.("starting");

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
  module.apply_world_command(JSON.stringify({ type: "set_is_game_master", isGameMaster }));
}

/**
 * Spec 014 (US4): forwards the per-die detail from an already-resolved
 * `rollDice` response into the engine's `DiceRollPlugin`, purely to
 * animate a reveal — this never asks the engine to decide an outcome
 * (FR-015). Not a `WorldCommand`/`worldStore` event: like
 * `setIsGameMaster`, this is a one-shot local trigger, not persisted
 * world state to sync or broadcast.
 */
export async function triggerDiceRollAnimation(dice: { finalValue: number }[]): Promise<void> {
  const module = await getWasmModule();
  if (!module.apply_world_command) {
    return;
  }
  module.apply_world_command(JSON.stringify({ type: "trigger_dice_roll", dice }));
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
