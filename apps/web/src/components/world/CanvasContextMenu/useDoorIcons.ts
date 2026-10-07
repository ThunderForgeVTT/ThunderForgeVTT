import { useEffect, useRef } from "react";
import { onDoorIconPressed } from "@/engine/bevy";
import type { WorldStore } from "@/engine/world/store";
import { doorIconAction } from "./canvasMenuActions";
import { performDoorAction } from "./doorActions";

/**
 * A press on a door's icon does what the right-click menu's matching item
 * does (spec 071 FR-009).
 *
 * The engine draws the icon and reports the press; `doorIconAction` says what
 * it means for this viewer, and `performDoorAction` carries it out — the same
 * rule and the same mutations the menu uses, so the icon can never offer what
 * the menu would not. A player's padlock changes nothing and says why.
 */
export function useDoorIcons(options: {
  worldStore: WorldStore;
  sceneId: string | null;
  isGameMaster: boolean;
  userId: string | null;
  onNotice: (notice: string) => void;
}): void {
  const { worldStore, sceneId, isGameMaster, userId } = options;
  // Held apart so a new callback each render does not resubscribe.
  const onNotice = useRef(options.onNotice);
  onNotice.current = options.onNotice;

  useEffect(() => {
    if (!sceneId) return;
    return onDoorIconPressed((event) => {
      const wall = worldStore.getState().walls[event.wallId];
      if (!wall) return;
      const action = doorIconAction({
        viewer: { isGameMaster, userId },
        wall,
      });
      if (!action) return;
      if (action.kind === "door-locked") {
        onNotice.current("It is locked.");
        return;
      }
      void performDoorAction(action, wall, {
        worldStore,
        sceneId,
        isGameMaster,
      }).then((problem) => {
        if (problem) onNotice.current(problem);
      });
    });
  }, [worldStore, sceneId, isGameMaster, userId]);
}
