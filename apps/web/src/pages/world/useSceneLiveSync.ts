import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from "react";
import {
  applyInteractiveWorldEvent,
  applyLightWorldEvent,
  applyShapeWorldEvent,
  applyTokenWorldEvent,
  applyTokenVisionWorldEvent,
  applyTokenGridWorldEvent,
  applyWallWorldEvent,
  subscribeToWorldEvents,
} from "@/engine/world/sync";
import { parseMapImportedEvent } from "@/engine/world/sync/scenes";
import {
  applyTokenStatusWorldEvent,
  refreshTokenStatus,
} from "@/engine/world/sync/tokenStatus";
import {
  applySceneLightingWorldEvent,
  asAmbientLevel,
  setSceneAmbient,
  type AmbientLevel,
} from "@/engine/world/sync/sceneLighting";
import {
  applySceneTimeWorldEvent,
  asHour,
  setSceneTime,
} from "@/engine/world/sync/sceneTime";
import type { WorldStore } from "@/engine/world/store";
import { setSceneTimeOfDay, updateSceneAmbientLight } from "@/api/scenes";
import type { SceneLevel } from "@/api/levels";
import type { SceneRecord } from "@/types/scene";
import type { AuthUser } from "@/types/auth";
import type { useSceneLevels } from "./useSceneLevels";

type SceneLevels = ReturnType<typeof useSceneLevels>;

interface UseSceneLiveSyncParams {
  id: string;
  sceneId: string | null;
  user: AuthUser | null;
  worldStore: WorldStore;
  bridgeReady: boolean;
  selectedScene: SceneRecord | null;
  sceneLevel: SceneLevel | null;
  sceneLevelList: readonly SceneLevel[];
  levelActions: SceneLevels["actions"];
  onLevelWorldEvent: SceneLevels["onWorldEvent"];
  sceneLevelRef: RefObject<SceneLevel | null>;
  sceneLevelListRef: RefObject<readonly SceneLevel[]>;
  reloadImportedSceneRef: RefObject<(importedSceneId: string) => void>;
  explorationEpochRef: RefObject<number>;
  setApprovalRevision: Dispatch<SetStateAction<number>>;
}

/**
 * The mounted scene's live sync, and its light.
 *
 * Moved out of `WorldPage` unchanged: the scene-scoped world-event loop that
 * keeps every client's board in step, the scene's ambient light as last heard,
 * and the control that changes it.
 */
export function useSceneLiveSync({
  id,
  sceneId,
  user,
  worldStore,
  bridgeReady,
  selectedScene,
  sceneLevel,
  sceneLevelList,
  levelActions,
  onLevelWorldEvent,
  sceneLevelRef,
  sceneLevelListRef,
  reloadImportedSceneRef,
  explorationEpochRef,
  setApprovalRevision,
}: UseSceneLiveSyncParams) {
  // Playtest 2026-09-10 P9: a scene's light as last heard — from a change
  // event or this Game Master's own control — ahead of the scene list, which
  // is fetched once and would otherwise go on holding the old level.
  //
  // Scene levels: the light is the *level's* now, and the level list is
  // re-read whenever a level changes — so what was last heard is trusted only
  // until the list it was heard against is replaced. `against` is that list;
  // a newer one carries the server's own answer and outranks it. Without that
  // an old "last heard" would go on masking a light another Game Master has
  // since changed, and would follow this one from floor to floor.
  const [ambientBySceneId, setAmbientBySceneId] = useState<
    Record<string, { level: AmbientLevel; against: readonly SceneLevel[] }>
  >({});
  const heardAmbient = sceneId ? ambientBySceneId[sceneId] : undefined;
  const sceneAmbient: AmbientLevel =
    (heardAmbient?.against === sceneLevelList
      ? heardAmbient.level
      : undefined) ??
    asAmbientLevel(sceneLevel?.ambientLight ?? selectedScene?.ambientLight);

  // Spec 099 US5: the scene's hour, as last heard, else as loaded. A heard
  // change goes to the engine with its transition as it arrives and is noted
  // in `sentTime`, so the effect below, which sets the hour at once when a
  // scene mounts, does not send it again as a jump.
  const [timeBySceneId, setTimeBySceneId] = useState<
    Record<string, number | null>
  >({});
  const heardTime = sceneId ? timeBySceneId[sceneId] : undefined;
  const sceneTime: number | null =
    heardTime !== undefined ? heardTime : asHour(selectedScene?.timeOfDay);
  const sentTime = useRef<{ sceneId: string; hour: number | null } | null>(
    null,
  );

  // Live cross-client sync: one subscription per mounted scene, feeding
  // every apply*WorldEvent for this scene's canvas primitives — each of
  // those already filters by its own event code internally (walls=10,
  // tokens=14, etc, see their own doc comments), so a single shared
  // subscription driving all four is correct and avoids opening four
  // separate WebSocket subscriptions for the same event stream. The
  // backend transport (Postgres listener -> broadcast channel -> this
  // `worldEventsCreated` GraphQL subscription -> /api/ws) already existed
  // in full; this is the first thing in apps/web to actually open it.
  useEffect(() => {
    if (!id || !sceneId || !bridgeReady) {
      return;
    }

    // Prime before the stream opens: the first paint should already carry
    // bars rather than waiting for something to change.
    void refreshTokenStatus(worldStore, sceneId).catch((error) => {
      console.error("Failed to load token status:", error);
    });

    // Not an AbortController + in-loop flag check: a `for await` loop
    // only re-checks anything between events, so if the world goes quiet
    // (no new events) the loop hangs forever awaiting the next one and
    // the subscription would never actually close on unmount/scene
    // change. Holding the iterator directly and calling `.return()` on
    // it unblocks that pending `next()` immediately (it synchronously
    // disposes the underlying graphql-ws subscription, which resolves
    // the pending promise via the `complete` callback) — this is why
    // `for await...of` itself calls `.return()` on early exit, and why a
    // manual cleanup needs to do the same explicitly.
    const iterator = subscribeToWorldEvents(id)[Symbol.asyncIterator]();
    let cancelled = false;

    (async () => {
      try {
        while (!cancelled) {
          const { value: event, done } = await iterator.next();
          if (done || cancelled || !event) break;
          // Scene levels: a level changed, or a token changed floor. Told
          // first and not awaited — it re-reads which level this viewer is on
          // and reloads the board itself if the answer moved.
          onLevelWorldEvent(event);
          // A map was imported into this scene. Nothing else announces its
          // walls, lights or art, so they are re-read here.
          if (parseMapImportedEvent(event) === sceneId) {
            reloadImportedSceneRef.current(sceneId);
          }
          await Promise.all([
            applyWallWorldEvent(worldStore, sceneId, event),
            applyTokenWorldEvent(worldStore, sceneId, event),
            applyShapeWorldEvent(worldStore, sceneId, event),
            applyLightWorldEvent(worldStore, sceneId, event),
            applyTokenStatusWorldEvent(worldStore, sceneId, event),
            applyInteractiveWorldEvent(worldStore, sceneId, event),
            // Spec 045 FR-067: a character's sheet moving changes what their
            // token can see, and until spec 045 a sheet edit announced
            // nothing at all.
            applyTokenVisionWorldEvent(worldStore, sceneId, event, () =>
              Object.keys(worldStore.getState().tokens),
            ),
            // Spec 046 US4: a sheet's size is the squares its tokens fill.
            applyTokenGridWorldEvent(worldStore, sceneId, event, () =>
              Object.keys(worldStore.getState().tokens),
            ),
          ]);
          // Spec 045 US7: a Game Master reset the fog. Re-read rather than
          // trusting the payload — a reset aimed at one player and one aimed
          // at everyone are the same event on the bus, and only the server
          // can say which applies to whoever is holding this browser.
          if (user?.id) {
            const epoch = await import("@/engine/world/sync/exploration").then(
              ({ applyExplorationWorldEvent }) =>
                applyExplorationWorldEvent(
                  user.id,
                  id,
                  sceneId,
                  event,
                  sceneLevelRef.current
                    ? {
                        levelId: sceneLevelRef.current.levelId,
                        isEntry: sceneLevelRef.current.isEntry,
                      }
                    : undefined,
                ),
            );
            if (epoch !== null) {
              explorationEpochRef.current = epoch;
            }
          }
          // Scene levels: the scene's own light is its entry level's. On
          // any other floor a change to it is news about a board this client
          // is not showing, and applying it would relight the wrong one.
          const shownLevel = sceneLevelRef.current;
          const ambient =
            shownLevel === null || shownLevel.isEntry
              ? applySceneLightingWorldEvent(worldStore, sceneId, event)
              : null;
          const time = applySceneTimeWorldEvent(worldStore, sceneId, event);
          if (time) {
            sentTime.current = { sceneId, hour: time.hour };
            setTimeBySceneId((current) => ({
              ...current,
              [sceneId]: time.hour,
            }));
          }
          if (ambient) {
            const against = sceneLevelListRef.current;
            setAmbientBySceneId((current) => ({
              ...current,
              [sceneId]: { level: ambient, against },
            }));
          }
          // The approval queue is server state, so a nudge on the bus is all
          // this side needs — it re-reads rather than holding a copy.
          setApprovalRevision((n) => n + 1);
        }
      } catch (error) {
        console.error("World events live-sync error:", error);
      }
    })();

    return () => {
      cancelled = true;
      void iterator.return?.();
    };
    // `user?.id` because the exploration handler above resets *this
    // player's* fog: a subscription that outlived a change of user would
    // carry the previous one's id and reset the wrong person's map.
    //
    // The refs and the setter are the page's own, so listing them changes
    // nothing: they are stable, and the stream still reopens only as before.
  }, [
    id,
    sceneId,
    bridgeReady,
    worldStore,
    user?.id,
    onLevelWorldEvent,
    reloadImportedSceneRef,
    sceneLevelRef,
    sceneLevelListRef,
    explorationEpochRef,
    setApprovalRevision,
  ]);

  // Playtest 2026-09-10 P9: hand the engine the scene's light. Nothing did,
  // so every scene rendered in daylight and no wall ever cast a shadow.
  useEffect(() => {
    if (!sceneId || !bridgeReady) {
      return;
    }
    setSceneAmbient(worldStore, sceneAmbient);
  }, [sceneId, bridgeReady, sceneAmbient, worldStore]);

  // Spec 099 US5: a scene mounts at its hour, at once. A scene with no hour
  // still sends `null`, so the last scene's clock does not follow it here.
  useEffect(() => {
    if (!sceneId || !bridgeReady) {
      sentTime.current = null;
      return;
    }
    const sent = sentTime.current;
    if (sent?.sceneId === sceneId && sent.hour === sceneTime) {
      return;
    }
    sentTime.current = { sceneId, hour: sceneTime };
    setSceneTime(worldStore, sceneTime);
  }, [sceneId, bridgeReady, sceneTime, worldStore]);

  // Applied when the server's event comes back, like everyone else's, so
  // the Game Master sees the same transition the table does.
  const changeSceneTime = useCallback(
    (hour: number | null, overSecs?: number) => {
      if (!sceneId) {
        return;
      }
      setSceneTimeOfDay(sceneId, hour, overSecs).catch((error: unknown) => {
        console.error("Failed to change the scene's time:", error);
      });
    },
    [sceneId],
  );

  const changeSceneAmbient = useCallback(
    (level: AmbientLevel) => {
      if (!sceneId) {
        return;
      }
      const previous = sceneAmbient;
      const against = sceneLevelList;
      setAmbientBySceneId((current) => ({
        ...current,
        [sceneId]: { level, against },
      }));
      if (sceneLevel) {
        // Scene levels: the control lights the floor on the board, not the
        // scene. No rollback to write — the level list is re-read whether the
        // server agreed or refused, and a re-read list outranks what was set
        // here a moment ago (see `ambientBySceneId`).
        void levelActions.setAmbient(sceneLevel.levelId, level);
        return;
      }
      updateSceneAmbientLight(sceneId, level).catch((error: unknown) => {
        console.error("Failed to change the scene's light:", error);
        setAmbientBySceneId((current) => ({
          ...current,
          [sceneId]: { level: previous, against },
        }));
      });
    },
    [sceneId, sceneAmbient, sceneLevel, sceneLevelList, levelActions],
  );

  return { sceneAmbient, changeSceneAmbient, sceneTime, changeSceneTime };
}
