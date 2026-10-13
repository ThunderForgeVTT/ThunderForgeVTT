import { asSdkError, SDK_VERSION } from "@/engine/sdk/commands";

import type { WorldStore } from "../world/store";
import type { WorldCommand } from "../world/types";
import {
  asAuthoringToolRevoked,
  asCanvasContextMenu,
  asDoorIconPressed,
  asGridSnapChanged,
  asInteractionTriggered,
  asInteractionUnavailable,
  asMovementBlocked,
  asOpenLore,
  asPickUpItem,
  asPlacementConfirmed,
  asSceneLoadRequested,
  authoringToolRevokedListeners,
  canvasContextMenuListeners,
  doorIconListeners,
  gridSnapListeners,
  movementBlockedListeners,
  pickUpItemListeners,
  placementCancelledListeners,
  placementConfirmedListeners,
  reportInteractionTriggered,
  reportOpenLore,
  reportSceneLoadRequested,
  reportSdkError,
  unavailableListeners,
} from "./engineEvents";
import { stopPeerTransfer } from "./peer";
import { getWasmModule } from "./wasmModule";

let worldStoreUnsubscribe: (() => void) | null = null;
let bevyCallbackRegistered = false;
let boundWorldStore: WorldStore | null = null;

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
        // Nor is a move stopped at a wall. Nothing changed — that is what it
        // reports — so there is no world state to put anywhere; it exists to
        // tell the player why their token did not move.
        // Nor is the snapping switch: it is the Game Master's for the
        // session, never scene data (spec 077).
        const snap = asGridSnapChanged(parsed);
        if (snap) {
          for (const listener of gridSnapListeners) {
            try {
              listener(snap);
            } catch {
              // One bad listener must not stop the others hearing about this.
            }
          }
          return;
        }

        const blocked = asMovementBlocked(parsed);
        if (blocked) {
          for (const listener of movementBlockedListeners) {
            try {
              listener(blocked);
            } catch {
              // One bad listener must not stop the others hearing about this.
            }
          }
          return;
        }

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
        // Nor is a right-click. It asks chrome to open a menu; nothing in the
        // world changed.
        const contextMenu = asCanvasContextMenu(parsed);
        if (contextMenu) {
          for (const listener of canvasContextMenuListeners) {
            try {
              listener(contextMenu);
            } catch {
              // One bad listener must not stop the others hearing about this.
            }
          }
          return;
        }

        // Nor is a door's icon: it asks chrome to do what the icon offers.
        // Heard once the engine's frame has returned: what the icon offers
        // dispatches into the store, which drives the engine, and the engine
        // is still inside the frame that reported the press.
        const doorIcon = asDoorIconPressed(parsed);
        if (doorIcon) {
          queueMicrotask(() => {
            for (const listener of doorIconListeners) {
              try {
                listener(doorIcon);
              } catch {
                // One bad listener must not stop the others hearing about this.
              }
            }
          });
          return;
        }

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

    // Spec 085: `set_walls_hidden` is the walls bridge's alone; the engine
    // hears the answer as `update_wall`. `delete_group` is the engine's own
    // Delete, so the bar and the key take one path.
    if (event.command.type === "set_walls_hidden") {
      return;
    }
    if (event.command.type === "delete_group") {
      module.delete_selection?.();
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
export function getBoundWorldStore(): WorldStore | null {
  return boundWorldStore;
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
