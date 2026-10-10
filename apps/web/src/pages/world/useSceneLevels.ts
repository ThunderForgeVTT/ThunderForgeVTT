import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { uploadCanvasImage } from "@/api/assets";
import {
  createSceneLevel,
  deleteSceneLevel,
  moveTokensToLevel,
  reorderSceneLevels,
  updateSceneLevel,
  type SceneLevel,
} from "@/api/levels";
import { setViewedLevel } from "@/api/viewedLevel";
import {
  levelEventKind,
  resolveLevelView,
  type WorldEventLike,
} from "@/engine/world/sync";

/**
 * Which level of the scene this page is showing, and the Game Master's means
 * of changing the levels there are.
 *
 * Kept out of `WorldPage` because it is one concern with one piece of state,
 * and that page already carries every other. What the page needs from it is
 * small: the level to load, whether that answer has arrived, and to be told
 * when it changes so it can reload the board.
 *
 * # The order things happen in when the level changes
 *
 * 1. `setViewedLevel` — synchronously, before anything renders. Every
 *    per-scene read consults it (`api/viewedLevel.ts`), so by the time the
 *    page's loaders run they are already asking for the new level.
 * 2. `onSwitch` — the page bumps its load generation, which withdraws the
 *    engine's grant, begins a transition for the same scene, and re-runs
 *    every loader once the engine has cleared the canvas. A level switch is a
 *    scene switch as far as the board is concerned.
 *
 * The first answer for a scene does *not* call `onSwitch`: the page holds its
 * first load until `settled`, so there is nothing on the board to reload.
 */

export interface SceneLevelActions {
  /** Show another level. A Game Master's tab; a player cannot choose. */
  select: (levelId: string) => void;
  add: (name: string) => Promise<void>;
  rename: (levelId: string, name: string) => Promise<void>;
  /** Move a level one place lower (`-1`) or higher (`1`). */
  shift: (levelId: string, by: -1 | 1) => Promise<void>;
  remove: (levelId: string) => Promise<void>;
  makeEntry: (levelId: string) => Promise<void>;
  /** `wallEdges`: wall the image's edges (spec 088 FR-093). Defaults to true. */
  setBackground: (
    levelId: string,
    file: Blob,
    wallEdges?: boolean,
  ) => Promise<void>;
  clearBackground: (levelId: string) => Promise<void>;
  /** `bright`, `dim` or `dark`. Resolves `false` when the server refused. */
  setAmbient: (levelId: string, ambientLight: string) => Promise<boolean>;
  moveTokens: (tokenIds: string[], levelId: string) => Promise<void>;
}

export interface SceneLevels {
  /** Every level this viewer may read, lowest first. Empty until settled. */
  levels: SceneLevel[];
  /** The level on screen, once known. */
  level: SceneLevel | null;
  /**
   * Whether the question has been answered for the current scene — with a
   * level, or with the news that it could not be read. The board's first load
   * waits on this, so a failed read must settle too or the scene never loads.
   */
  settled: boolean;
  /** Feed every world event of the scene through this. */
  onWorldEvent: (event: WorldEventLike) => void;
  actions: SceneLevelActions;
}

interface Shown {
  sceneId: string;
  levels: SceneLevel[];
  levelId: string | null;
}

function refusal(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useSceneLevels({
  worldId,
  sceneId,
  userId,
  isGm,
  onSwitch,
}: {
  worldId: string | undefined;
  sceneId: string | null;
  userId: string | null;
  isGm: boolean;
  /** The level on screen changed for a scene already on the board. */
  onSwitch: () => void;
}): SceneLevels {
  const [shown, setShown] = useState<Shown | null>(null);
  /** The scene a read failed for, so the page can stop waiting on it. */
  const [failedFor, setFailedFor] = useState<string | null>(null);

  /**
   * What the latest render asked for, readable from a read already in flight.
   *
   * A read that began for one scene can finish after the page has moved to
   * another, and a second request arriving while one is in flight is folded
   * into it rather than started beside it. Both need "what is wanted *now*",
   * not what the closure that started the read captured.
   */
  const wanted = useRef({ sceneId, userId, isGm });
  /** What was last put on screen — the thing a new answer is compared with. */
  const shownRef = useRef<{ sceneId: string; levelId: string | null } | null>(
    null,
  );
  /** A Game Master's chosen tab, per scene, for as long as the page lives. */
  const picked = useRef(new Map<string, string>());
  const reading = useRef(false);
  const again = useRef(false);
  const onSwitchRef = useRef(onSwitch);
  useEffect(() => {
    onSwitchRef.current = onSwitch;
  }, [onSwitch]);

  const show = useCallback(
    (forScene: string, levels: SceneLevel[], levelId: string | null) => {
      const before = shownRef.current;
      setViewedLevel(forScene, levelId);
      shownRef.current = { sceneId: forScene, levelId };
      setShown({ sceneId: forScene, levels, levelId });
      if (before?.sceneId === forScene && before.levelId !== levelId) {
        onSwitchRef.current();
      }
    },
    [],
  );

  const refresh = useCallback(async () => {
    if (reading.current) {
      again.current = true;
      return;
    }
    reading.current = true;
    try {
      do {
        again.current = false;
        const asked = wanted.current;
        if (!asked.sceneId) {
          return;
        }
        const askedScene = asked.sceneId;
        try {
          const view = await resolveLevelView(
            askedScene,
            { userId: asked.userId, isGm: asked.isGm },
            picked.current.get(askedScene) ?? null,
          );
          if (wanted.current.sceneId === askedScene) {
            show(askedScene, view.levels, view.levelId);
          }
        } catch (error) {
          // Not fatal, and not silent. The board falls back to what it did
          // before levels existed — the scene's own art, and whichever level
          // the server opens on — which is right for every one-level scene.
          console.error("Failed to read the scene's levels:", error);
          if (wanted.current.sceneId === askedScene) {
            setFailedFor(askedScene);
          }
        }
        // A different scene was asked for while this one was being read.
        if (wanted.current.sceneId !== askedScene) {
          again.current = true;
        }
      } while (again.current);
    } finally {
      reading.current = false;
    }
  }, [show]);

  useEffect(() => {
    wanted.current = { sceneId, userId, isGm };
    if (sceneId) {
      void refresh();
    }
  }, [sceneId, userId, isGm, refresh]);

  const onWorldEvent = useCallback(
    (event: WorldEventLike) => {
      const asked = wanted.current;
      if (!asked.sceneId) {
        return;
      }
      const kind = levelEventKind(event, asked.sceneId);
      if (kind === null) {
        return;
      }
      if (asked.isGm && kind === "token") {
        // A Game Master's level is the tab they picked, whatever any token
        // does. Only a token arriving or leaving changes anything they are
        // shown — the count on each tab.
        const action = (
          (event.token_event ?? event.tokenEvent) as
            | { action?: string }
            | undefined
        )?.action;
        if (action !== "created" && action !== "deleted") {
          return;
        }
      }
      void refresh();
    },
    [refresh],
  );

  const current = shown && shown.sceneId === sceneId ? shown : null;
  const levels = useMemo(() => current?.levels ?? [], [current]);
  const level = useMemo(
    () => levels.find((each) => each.levelId === current?.levelId) ?? null,
    [levels, current],
  );

  const actions = useMemo((): SceneLevelActions => {
    /** Run one change, say so if it was refused, and re-read either way. */
    const change = async (
      failed: string,
      run: () => Promise<unknown>,
    ): Promise<boolean> => {
      try {
        await run();
        return true;
      } catch (error) {
        // The server's own words: "A level with tokens on it cannot be
        // deleted" is written to be read by a Game Master.
        toast.error(`${failed}: ${refusal(error)}`);
        return false;
      } finally {
        await refresh();
      }
    };

    return {
      select: (levelId) => {
        const asked = wanted.current;
        const here = shownRef.current;
        if (!asked.sceneId || !asked.isGm || here?.sceneId !== asked.sceneId) {
          return;
        }
        picked.current.set(asked.sceneId, levelId);
        void refresh();
      },
      add: async (name) => {
        if (!sceneId) return;
        await change("Could not add the level", () =>
          createSceneLevel({ sceneId, name }),
        );
      },
      rename: async (levelId, name) => {
        await change("Could not rename the level", () =>
          updateSceneLevel(levelId, { name }),
        );
      },
      shift: async (levelId, by) => {
        if (!sceneId) return;
        const order = levels.map((each) => each.levelId);
        const from = order.indexOf(levelId);
        const to = from + by;
        if (from < 0 || to < 0 || to >= order.length) return;
        [order[from], order[to]] = [order[to], order[from]];
        await change("Could not reorder the levels", () =>
          reorderSceneLevels(sceneId, order),
        );
      },
      remove: async (levelId) => {
        await change("Could not delete the level", () =>
          deleteSceneLevel(levelId),
        );
      },
      makeEntry: async (levelId) => {
        await change("Could not change where the scene opens", () =>
          updateSceneLevel(levelId, { makeEntry: true }),
        );
      },
      setBackground: async (levelId, file, wallEdges = true) => {
        if (!worldId || !sceneId) return;
        await change("Could not set the level's map", async () => {
          const asset = await uploadCanvasImage(
            worldId,
            sceneId,
            "BACKGROUND",
            file,
          );
          // The level takes the art's own size, as a scene does when a map is
          // imported: art stretched over a board of another shape puts every
          // wall drawn against it in the wrong place.
          await updateSceneLevel(levelId, {
            backgroundAssetId: asset.id,
            width: asset.widthPx,
            height: asset.heightPx,
            wallEdges,
          });
        });
      },
      clearBackground: async (levelId) => {
        await change("Could not remove the level's map", () =>
          updateSceneLevel(levelId, { clearBackground: true }),
        );
      },
      setAmbient: (levelId, ambientLight) =>
        change("Could not change the level's light", () =>
          updateSceneLevel(levelId, { ambientLight }),
        ),
      moveTokens: async (tokenIds, levelId) => {
        if (tokenIds.length === 0) return;
        await change("Could not move the tokens", () =>
          moveTokensToLevel(tokenIds, levelId),
        );
      },
    };
  }, [levels, refresh, sceneId, worldId]);

  return {
    levels,
    level,
    settled: sceneId !== null && (current !== null || failedFor === sceneId),
    onWorldEvent,
    actions,
  };
}
