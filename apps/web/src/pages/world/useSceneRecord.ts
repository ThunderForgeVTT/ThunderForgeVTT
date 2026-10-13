import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import { getWorld } from "@/api/world";
import { getScene, getScenes } from "@/api/scenes";
import type { WorldRecord } from "@/types/world";
import type { SceneRecord } from "@/types/scene";

interface UseSceneRecordParams {
  id: string;
  sceneId: string | null;
  scenes: SceneRecord[];
  scenesFetchedForWorld: string | null;
  setWorld: Dispatch<SetStateAction<WorldRecord | null>>;
  setSelectedSceneId: Dispatch<SetStateAction<string | null>>;
  setScenes: Dispatch<SetStateAction<SceneRecord[]>>;
  setScenesFetchedForWorld: Dispatch<SetStateAction<string | null>>;
}

/**
 * The world, its scene list, and the record of the scene being played.
 *
 * Moved out of `WorldPage` unchanged. The world and scene list stay the
 * page's state — the page reads them long before this runs — so their setters
 * are passed in; the by-id record of the active scene is this hook's own.
 */
export function useSceneRecord({
  id,
  sceneId,
  scenes,
  scenesFetchedForWorld,
  setWorld,
  setSelectedSceneId,
  setScenes,
  setScenesFetchedForWorld,
}: UseSceneRecordParams) {
  // The scene being played, fetched by id when the list does not carry it.
  //
  // `scenes(worldId:)` filters hidden scenes out for non-GMs (spec 022
  // FR-008), while `sceneId` comes from the world's unfiltered
  // `activeSceneId` — and a scene is hidden by default when created
  // (FR-003). So a player playing a scene the GM never un-hid had a scene
  // id and no scene record for the whole session, and the record is what
  // carries the map art and the grid: their canvas silently had neither.
  // The server now lets a member read the one scene their world is actually
  // playing, and this is the fetch that asks for it.
  const [activeSceneRecord, setActiveSceneRecord] =
    useState<SceneRecord | null>(null);
  // Which scene id we have finished trying to find a record for, whether or
  // not we found one. The scene-load state below needs "we looked and there
  // is nothing" — resolving on "we have not looked yet" would clear the
  // loading overlay before the answer was known.
  // Only the by-id fetch's outcome is state. A scene the world's own list
  // already contains has been "looked for" by definition, and that is
  // derived below rather than written from inside the effect that notices
  // it (react-hooks/set-state-in-effect).
  const [sceneRecordFetchedFor, setSceneRecordFetchedFor] = useState<
    string | null
  >(null);
  const sceneIsInWorldList = scenes.some((scene) => scene.sceneId === sceneId);
  const selectedScene =
    scenes.find((scene) => scene.sceneId === sceneId) ??
    (activeSceneRecord?.sceneId === sceneId ? activeSceneRecord : null);
  const sceneRecordSettledFor = sceneIsInWorldList
    ? sceneId
    : sceneRecordFetchedFor;

  useEffect(() => {
    if (!id) {
      return;
    }

    let active = true;

    void getWorld(id)
      .then((response) => {
        if (active) {
          setWorld(response);
          // Spec 022 (FR-002d, ADR-046): scene selection is now
          // server-authoritative (`world.activeSceneId`), not "default to
          // the first scene" — a world nothing has ever been launched in
          // shows the empty/unloaded canvas state rather than picking one
          // arbitrarily. The functional update only fires if nothing has
          // set `selectedSceneId` yet (e.g. a live launch event, handled
          // elsewhere), so this never clobbers a switch already in flight.
          setSelectedSceneId(
            (current) => current ?? response?.activeSceneId ?? null,
          );
        }
      })
      .catch(() => {
        if (active) {
          setWorld(null);
        }
      });

    return () => {
      active = false;
    };
    // The setters are the page's `useState` setters: stable, so this still
    // runs once per world.
  }, [id, setWorld, setSelectedSceneId]);

  useEffect(() => {
    if (!id) {
      return;
    }

    let active = true;

    void getScenes(id)
      .then((response) => {
        if (!active) {
          return;
        }
        setScenes(response);
        setScenesFetchedForWorld(id);
      })
      .catch((error) => {
        console.error("Failed to load world scenes:", error);
        if (active) {
          setScenes([]);
          setScenesFetchedForWorld(id);
        }
      });

    return () => {
      active = false;
    };
  }, [id, setScenes, setScenesFetchedForWorld]);

  // Only when the list came back without it — a GM's list already has every
  // scene, so this is one extra request for players and none for a GM.
  useEffect(() => {
    if (!sceneId || scenesFetchedForWorld !== id) {
      return;
    }
    // Already in the world's list — no by-id fetch to make, and
    // `sceneRecordSettledFor` above already reads as settled for it.
    if (sceneIsInWorldList) {
      return;
    }
    if (activeSceneRecord?.sceneId === sceneId) {
      return;
    }

    let active = true;
    void getScene(sceneId)
      .then((scene) => {
        if (active && scene) {
          setActiveSceneRecord(scene);
        }
      })
      .catch((error) => {
        // Non-fatal, and deliberately not an error to the user: the canvas
        // still renders tokens, walls and lights, which load on their own.
        // What is lost is the map art and the grid, and there is nothing
        // for anyone to do about it here.
        console.error("Failed to load the active scene:", error);
      })
      .finally(() => {
        if (active) {
          setSceneRecordFetchedFor(sceneId);
        }
      });

    return () => {
      active = false;
    };
  }, [
    sceneId,
    sceneIsInWorldList,
    scenesFetchedForWorld,
    id,
    activeSceneRecord,
  ]);

  return { selectedScene, sceneRecordSettledFor, setActiveSceneRecord };
}
