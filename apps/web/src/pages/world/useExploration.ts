import { useEffect, type RefObject } from "react";
import type { SceneLevel } from "@/api/levels";

interface UseExplorationPersistenceParams {
  id: string;
  sceneId: string | null;
  userId: string | undefined;
  engineReady: boolean;
  sceneLevel: SceneLevel | null;
  sceneLevelSettled: boolean;
  explorationEpoch: RefObject<number>;
}

/**
 * Keeps what this player has explored in their own browser (spec 045 US7).
 *
 * Moved out of `WorldPage` unchanged. The exploration epoch stays the page's
 * ref, because the world-event loop also writes it when a Game Master resets
 * the fog.
 */
export function useExplorationPersistence({
  id,
  sceneId,
  userId,
  engineReady,
  sceneLevel,
  sceneLevelSettled,
  explorationEpoch,
}: UseExplorationPersistenceParams) {
  const explorationLevelId = sceneLevel?.levelId ?? null;
  const explorationLevelIsEntry = sceneLevel?.isEntry ?? false;
  useEffect(() => {
    // Scene levels: what a player has explored is remembered per floor, so
    // this waits to be told which floor. Starting on the scene's own key and
    // moving to the level's a moment later would save the first under the
    // second's name on the way out.
    if (!engineReady || !id || !sceneId || !userId || !sceneLevelSettled) {
      return;
    }
    const worldId = id;
    const scene = sceneId;
    const level = explorationLevelId
      ? { levelId: explorationLevelId, isEntry: explorationLevelIsEntry }
      : undefined;
    let stopped = false;

    void import("@/engine/world/sync/exploration").then(
      async ({ loadExploration }) => {
        const epoch = await loadExploration(userId, worldId, scene, level);
        if (!stopped) {
          explorationEpoch.current = epoch;
        }
      },
    );

    const timer = window.setInterval(() => {
      void import("@/engine/world/sync/exploration").then(
        ({ saveExploration }) =>
          saveExploration(
            userId,
            worldId,
            scene,
            explorationEpoch.current,
            level,
          ),
      );
    }, 10_000);

    return () => {
      stopped = true;
      window.clearInterval(timer);
      // One last save on the way out, so leaving a scene deliberately does
      // not cost the walk that led to leaving it.
      void import("@/engine/world/sync/exploration").then(
        ({ saveExploration }) =>
          saveExploration(
            userId,
            worldId,
            scene,
            explorationEpoch.current,
            level,
          ),
      );
    };
  }, [
    engineReady,
    id,
    sceneId,
    userId,
    sceneLevelSettled,
    explorationLevelId,
    explorationLevelIsEntry,
    // The page's `useRef`: stable, so it never re-runs this on its own.
    explorationEpoch,
  ]);
}
