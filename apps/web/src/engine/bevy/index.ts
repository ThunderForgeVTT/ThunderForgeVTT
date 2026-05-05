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

export async function mountEngine(options: EngineMountOptions): Promise<void> {
  const module = await getWasmModule();

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
