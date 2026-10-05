import {
  getInteractives,
  refusalNotice,
  setDoorDesignation,
  setDoorLock,
  setDoorSecret,
} from "@/api/interactives";
import { activateAndApply } from "@/engine/world/sync/interactives";
import type { WorldStore } from "@/engine/world/store";
import type { WorldWall } from "@/engine/world/types";
import type { DoorMenuAction } from "./canvasMenuActions";

/**
 * Carrying out what the door menu offered (spec 071).
 *
 * Every one of these is decided by the server: the Game Master's by who runs
 * the world, a player's by the door's interactive and its lock. Nothing here
 * checks first, and what comes back is only what to tell the person when it
 * did not go through — `null` when the door changing is the answer.
 */

/** The interactive a door opens by, for this viewer; `null` when it has none. */
export async function doorInteractive(sceneId: string, wallId: string) {
  const interactives = await getInteractives(sceneId).catch(() => []);
  return (
    interactives.find(
      (interactive) =>
        interactive.subjectKind === "door" && interactive.subjectRef === wallId,
    ) ?? null
  );
}

/**
 * Shut, locked and hidden, in that order.
 *
 * Shut first: a hidden door standing open is a hole in a wall the table can
 * see through and cannot see. Hidden last, so a failure part way leaves a
 * locked door in view rather than a hidden one that opens.
 */
async function lockAsWall(worldStore: WorldStore, wall: WorldWall) {
  if (wall.doorState === "open") {
    worldStore.dispatch(
      {
        type: "update_wall",
        wallId: wall.id,
        changes: { doorState: "closed" },
      },
      "ui",
    );
  }
  if (!wall.locked) await setDoorLock(wall.id, true);
  if (!wall.secret) await setDoorSecret(wall.id, true);
}

export async function performDoorAction(
  action: DoorMenuAction,
  wall: WorldWall,
  context: {
    worldStore: WorldStore;
    sceneId: string;
    isGameMaster: boolean;
  },
): Promise<string | null> {
  const { worldStore, sceneId, isGameMaster } = context;
  try {
    switch (action.kind) {
      case "door-state":
        if (isGameMaster) {
          // Set, not toggled: the menu said which, and a toggle sent twice
          // would be a door back where it started.
          worldStore.dispatch(
            {
              type: "update_wall",
              wallId: wall.id,
              changes: { doorState: action.open ? "open" : "closed" },
            },
            "ui",
          );
          return null;
        } else {
          const interactive = await doorInteractive(sceneId, wall.id);
          if (!interactive) return "That did not work.";
          return refusalNotice(
            await activateAndApply(worldStore, interactive.interactiveId),
          );
        }
      case "door-lock":
        await setDoorLock(wall.id, action.locked);
        return null;
      case "door-gm-lock":
        await lockAsWall(worldStore, wall);
        return null;
      case "door-reveal":
        await setDoorSecret(wall.id, false);
        return null;
      case "door-designate":
        await setDoorDesignation(wall.id, action.isDoor);
        return null;
      case "door-locked":
        return null;
    }
  } catch (error) {
    return error instanceof Error && error.message
      ? error.message
      : "That did not go through.";
  }
}
