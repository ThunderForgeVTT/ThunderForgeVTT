import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type RefObject,
} from "react";
import type { SceneLevel } from "@/api/levels";
import type { useEngineBridge } from "./useEngineBridge";

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

interface UseExplorationControlsParams {
  sceneId: string | null;
  isSceneOwner: boolean;
  allTokens: ReturnType<typeof useEngineBridge>["allTokens"];
}

/**
 * The Game Master's exploration controls (spec 045 US7).
 *
 * Moved out of `WorldPage` unchanged: whether the scene remembers, who may be
 * reset individually, and the toggle and reset themselves.
 */
export function useExplorationControls({
  sceneId,
  isSceneOwner,
  allTokens,
}: UseExplorationControlsParams) {
  // Spec 045 US7: the Game Master's two controls — whether this scene
  // remembers, and resetting what it has remembered.
  const [explorationOn, setExplorationOn] = useState(false);
  useEffect(() => {
    if (!sceneId || !isSceneOwner) {
      return;
    }
    let stale = false;
    void import("@/engine/world/sync/exploration")
      .then(({ getSceneExploration }) => getSceneExploration(sceneId))
      .then((scene) => {
        if (!stale) {
          setExplorationOn(scene.enabled);
        }
      })
      .catch(() => {
        // A control that cannot read its own state shows off, which is the
        // scene's default and the safe thing to show.
      });
    return () => {
      stale = true;
    };
  }, [sceneId, isSceneOwner]);

  // Who a Game Master may reset individually.
  //
  // Derived from the tokens that have owners, labelled by the token's own
  // name: it is the name on the board the Game Master is looking at, and it
  // is what they would say out loud. A world-membership list would be more
  // complete and less useful — it would offer players who have no token in
  // this scene and therefore no fog to reset.
  const explorationPlayers = useMemo(() => {
    const byOwner = new Map<string, string>();
    for (const token of Object.values(allTokens)) {
      if (token.ownerUserId && !byOwner.has(token.ownerUserId)) {
        byOwner.set(token.ownerUserId, token.label ?? "a player");
      }
    }
    return [...byOwner].map(([userId, name]) => ({ userId, name }));
  }, [allTokens]);

  const changeExploration = useCallback(
    (enabled: boolean) => {
      if (!sceneId) {
        return;
      }
      // Shown immediately and put back if the server refuses, like the
      // scene's light above: a toggle that waits for a round trip feels
      // broken even when it works.
      setExplorationOn(enabled);
      void import("@/api/exploration")
        .then(({ setSceneExploration }) =>
          setSceneExploration(sceneId, enabled),
        )
        .catch((error: unknown) => {
          console.error("Failed to change exploration:", error);
          setExplorationOn(!enabled);
        });
    },
    [sceneId],
  );

  const resetExploration = useCallback(
    (forUser: string | null) => {
      if (!sceneId) {
        return;
      }
      void import("@/api/exploration")
        .then(({ resetSceneExploration }) =>
          resetSceneExploration(sceneId, forUser),
        )
        .catch((error: unknown) => {
          console.error("Failed to reset exploration:", error);
        });
    },
    [sceneId],
  );

  return {
    explorationOn,
    explorationPlayers,
    changeExploration,
    resetExploration,
  };
}
