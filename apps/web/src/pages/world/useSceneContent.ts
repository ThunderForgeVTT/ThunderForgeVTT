import { useEffect } from "react";
import {
  loadLightsIntoStore,
  loadShapesIntoStore,
  loadTokensIntoStore,
  loadWallsIntoStore,
  startLightMutationBridge,
  startShapeMutationBridge,
  startTokenMutationBridge,
  startWallMutationBridge,
} from "@/engine/world/sync";
import type { WorldStore } from "@/engine/world/store";
import type { SceneLevel } from "@/api/levels";
import type { SceneRecord } from "@/types/scene";
import type { SceneLoadResource } from "./useSceneLoadState";

interface UseSceneContentParams {
  id: string;
  sceneId: string | null;
  worldStore: WorldStore;
  isSceneOwner: boolean;
  selectedScene: SceneRecord | null;
  sceneRecordSettledFor: string | null;
  sceneLevel: SceneLevel | null;
  bridgeReady: boolean;
  sceneLoadGranted: boolean;
  sceneLoadGeneration: number;
  markSceneResourceLoaded: (
    resource: SceneLoadResource,
    generation: number,
  ) => void;
  markSceneResourceFailed: (
    resource: SceneLoadResource,
    generation: number,
  ) => void;
}

/**
 * Loads the granted scene onto the board: its background and grid, then its
 * walls, tokens, lights and shapes, each with its mutation bridge.
 *
 * Moved out of `WorldPage` unchanged. Every loader waits for the engine's
 * grant (`useSceneTransition`) and reports into the scene-load state.
 */
export function useSceneContent({
  id,
  sceneId,
  worldStore,
  isSceneOwner,
  selectedScene,
  sceneRecordSettledFor,
  sceneLevel,
  bridgeReady,
  sceneLoadGranted,
  sceneLoadGeneration,
  markSceneResourceLoaded,
  markSceneResourceFailed,
}: UseSceneContentParams) {
  // The level's board, as plain values rather than the level itself: the level
  // list is re-read on every token that arrives or leaves (a Game Master's
  // tabs count them), and a new object for an unchanged floor must not send
  // the engine its map again.
  const hasLevelBoard = sceneLevel !== null;
  const levelBackgroundUrl = sceneLevel?.backgroundUrl ?? null;
  const levelWidth = sceneLevel?.width;
  const levelHeight = sceneLevel?.height;

  useEffect(() => {
    // Gated on the engine's grant (see `sceneLoadGrant`): the background and
    // the grid are scene content like everything else, and re-pointing the
    // sprite before the despawn frame would show the new map under the old
    // scene's walls.
    if (!bridgeReady || !sceneLoadGranted) {
      return;
    }

    // No scene record at all, after we have finished looking for one.
    //
    // This used to be every player: `scenes(worldId:)` filters hidden
    // scenes out for non-GMs (spec 022 FR-008) while `selectedSceneId`
    // comes from the world's unfiltered `activeSceneId`, so `selectedScene`
    // stayed null for the whole session, the dispatch below never ran, and
    // "background" was never marked — the pending set never emptied and
    // players sat under a permanent "Loading scene…" over a canvas that had
    // in fact fully loaded. That is fixed at the source now: a member may
    // read the scene their world is playing, and `activeSceneRecord` above
    // fetches it. What remains is the genuine case — there really is no
    // record to be had — where there is no `backgroundUrl` for anything to
    // load, so the resource is satisfied rather than left pending, the same
    // call the no-art branch below makes.
    //
    // Gated on having *finished looking*, not on the world's scene list
    // arriving: resolving while the by-id fetch is still in flight would
    // clear the overlay before the answer was known.
    if (!selectedScene) {
      if (sceneId && sceneRecordSettledFor === sceneId) {
        markSceneResourceLoaded("background", sceneLoadGeneration);
      }
      return;
    }

    const boardBackgroundUrl = hasLevelBoard
      ? levelBackgroundUrl
      : selectedScene.backgroundUrl;
    const boardWidth = levelWidth ?? selectedScene.width;
    const boardHeight = levelHeight ?? selectedScene.height;

    // Switching scenes re-points the engine's background sprite at the
    // newly-selected scene's imported map art (null clears it for a scene
    // with no import). worldStore.dispatch's generic bindWorldStore
    // forwarder relays this to the engine the same way every other
    // WorldCommand is relayed — no direct engine-bridge call needed.
    // Gated on `bridgeReady` (see its declaration) so this dispatch isn't
    // lost to the bindWorldStore-registration race.
    // Bug fix: this used to read `backgroundImagePath` directly, which
    // dd2vtt/map import (spec 022/002) never sets — it writes the
    // background to RustFS via `background_asset_id` instead, so an
    // imported map's art silently never loaded into the canvas.
    // `backgroundUrl` (GraphQLScene, graphql.rs) is the fetchable URL for
    // whichever mechanism actually populated the scene, computed
    // server-side — use that here instead.
    //
    // Scene levels: the art and its extent are the *level's*. The scene's own
    // are its entry level's, and a player standing on any other floor is
    // refused them outright — so the scene's are used only when no level
    // could be read at all.
    worldStore.dispatch(
      {
        type: "set_scene_background",
        backgroundImagePath: boardBackgroundUrl,
        width: boardWidth,
        height: boardHeight,
        worldId: id,
      },
      "ui",
    );

    // Sent with the background, never separately: the grid the engine draws
    // and snaps to has to be the one this scene's art was authored against.
    // Before this the engine had no way to learn a scene's grid at all — it
    // drew a hardcoded lattice and snapped to a different hardcoded one, so
    // an imported dd2vtt's `pixels_per_grid` (commonly 128) was ignored on
    // both counts.
    worldStore.dispatch(
      {
        type: "set_scene_grid",
        gridType: selectedScene.gridType,
        size: selectedScene.gridSize,
        // The scene's extent, so the engine anchors the lattice to the map's
        // corner rather than the world origin. The background sprite is
        // centred on the origin, so an origin-anchored grid only lines up with
        // the art when the map is an even number of cells across — a coin
        // flip, and wrong for half this project's own example maps.
        mapWidth: boardWidth,
        mapHeight: boardHeight,
        visible: true,
      },
      "ui",
    );

    // Spec 004 (US4): a scene with no imported art has nothing to fail —
    // mark "background" satisfied immediately. A scene with art gets a
    // real reachability check (the engine loads the sprite itself with no
    // JS-visible success/failure signal, so this is the only way to
    // surface "the background asset is unreachable" per FR-013 rather
    // than leaving the canvas silently blank).
    const generation = sceneLoadGeneration;
    if (!boardBackgroundUrl) {
      markSceneResourceLoaded("background", generation);
      return;
    }

    let cancelled = false;
    void fetch(boardBackgroundUrl, {
      method: "HEAD",
      credentials: "same-origin",
    })
      .then((response) => {
        if (cancelled) {
          return;
        }
        if (response.ok) {
          markSceneResourceLoaded("background", generation);
        } else {
          markSceneResourceFailed("background", generation);
        }
      })
      .catch(() => {
        if (!cancelled) {
          markSceneResourceFailed("background", generation);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [
    selectedScene,
    sceneId,
    sceneRecordSettledFor,
    worldStore,
    id,
    bridgeReady,
    sceneLoadGranted,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
    hasLevelBoard,
    levelBackgroundUrl,
    levelWidth,
    levelHeight,
  ]);

  useEffect(() => {
    // `sceneLoadGranted`: the engine has finished taking the previous scene
    // off the canvas and is waiting for this one. Loading before that races
    // its despawn — see `sceneLoadGrant` above.
    if (!sceneId || !bridgeReady || !sceneLoadGranted) {
      return;
    }

    // Kept, with a narrower job than it used to have.
    //
    // The canvas is no longer this loop's responsibility: spec 031's
    // transition machine despawns the previous scene's walls (with its
    // tokens, lights, shapes and selection) before the grant that let this
    // effect run at all, and Constitution Principle I says one owner — the
    // engine owns what is on the canvas. Removing this outright was the
    // considered alternative, and it is wrong: `worldStore` is chrome's own
    // copy, not a view of the engine's, and the engine cannot reach into it.
    // `loadWallsIntoStore` only ever upserts, so without this the previous
    // scene's rows accumulate in chrome's state for the life of the session —
    // invisible on the canvas now, but read by every panel that lists walls.
    //
    // `"sync"` source: a confirmed-state correction, not a delete intent —
    // startWallMutationBridge below ignores `source: "sync"` dispatches, so
    // this never calls deleteWall. It is relayed on into the engine as well,
    // where it lands on entities the despawn already took: a no-op, and
    // cheaper to leave alone than to build a second dispatch path that
    // skips the engine.
    for (const wallId of Object.keys(worldStore.getState().walls)) {
      worldStore.dispatch({ type: "remove_wall", wallId }, "sync");
    }

    // Gated on `bridgeReady` (see its declaration): without it, this can
    // fire and dispatch the confirmed walls before bindWorldStore has
    // registered its forwarding subscription, silently losing them from
    // the engine's WallSet even though they're genuinely persisted.
    const generation = sceneLoadGeneration;
    void loadWallsIntoStore(worldStore, sceneId)
      .then(() => markSceneResourceLoaded("walls", generation))
      .catch((error) => {
        console.error("Failed to load scene walls:", error);
        markSceneResourceFailed("walls", generation);
      });

    const stopBridge = startWallMutationBridge(worldStore, sceneId);

    return () => {
      stopBridge();
    };
  }, [
    sceneId,
    worldStore,
    bridgeReady,
    sceneLoadGranted,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);

  useEffect(() => {
    // `sceneLoadGranted`: the engine has finished taking the previous scene
    // off the canvas and is waiting for this one. Loading before that races
    // its despawn — see `sceneLoadGrant` above.
    if (!sceneId || !bridgeReady || !sceneLoadGranted) {
      return;
    }

    // Chrome's own copy of the tokens the canvas no longer holds, cleared for
    // the same reason as the walls and shapes loops above. Spec 031's
    // transition despawns every token on the canvas, so without this the store
    // keeps rows for tokens that no longer exist there: a panel listing one, a
    // party roster placing one, and a click at its reported position landing on
    // empty canvas. (The engine's demo tokens, which once made this worst, are
    // spawned only in the engine sandbox now.)
    //
    // `loadTokensIntoStore` only upserts, so leaving the stale rows for it to
    // overwrite was never an option — nothing overwrites a row whose id the
    // new scene never mentions. `"sync"` source: a confirmed-state correction,
    // not a delete intent, and `startTokenMutationBridge` returns early on
    // `"sync"`, so this never calls `deleteToken`.
    for (const tokenId of Object.keys(worldStore.getState().tokens)) {
      worldStore.dispatch({ type: "remove_token", tokenId }, "sync");
    }

    const tokensGeneration = sceneLoadGeneration;
    void loadTokensIntoStore(worldStore, sceneId)
      .then(() => markSceneResourceLoaded("tokens", tokensGeneration))
      .catch((error) => {
        console.error("Failed to load scene tokens:", error);
        markSceneResourceFailed("tokens", tokensGeneration);
      });

    const stopBridge = startTokenMutationBridge(
      worldStore,
      sceneId,
      isSceneOwner,
      // Spec 028 US7: the bridge needs the world to queue an edit against
      // when there is nowhere to send it.
      id,
    );

    return () => {
      stopBridge();
    };
  }, [
    sceneId,
    worldStore,
    bridgeReady,
    sceneLoadGranted,
    isSceneOwner,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
    // The world the token bridge queues offline edits against (spec 028 US7).
    id,
  ]);

  useEffect(() => {
    // `sceneLoadGranted`: the engine has finished taking the previous scene
    // off the canvas and is waiting for this one. Loading before that races
    // its despawn — see `sceneLoadGrant` above.
    if (!sceneId || !bridgeReady || !sceneLoadGranted) {
      return;
    }

    // Chrome's own copy of the previous board's lights, cleared for the same
    // reason as the walls, tokens and shapes loops. It was the one loader
    // without such a loop, and a level switch is what made that matter: the
    // floor left behind and the floor arrived at are one scene, so nothing
    // else would ever take the old floor's lights out of the Lights panel.
    // `"sync"` source, so the light bridge never calls `deleteLight`.
    for (const lightId of Object.keys(worldStore.getState().lights)) {
      worldStore.dispatch({ type: "remove_light", lightId }, "sync");
    }

    const lightsGeneration = sceneLoadGeneration;
    void loadLightsIntoStore(worldStore, sceneId)
      .then(() => markSceneResourceLoaded("lights", lightsGeneration))
      .catch((error) => {
        console.error("Failed to load scene lights:", error);
        markSceneResourceFailed("lights", lightsGeneration);
      });

    const stopBridge = startLightMutationBridge(worldStore, sceneId);

    return () => {
      stopBridge();
    };
  }, [
    sceneId,
    worldStore,
    bridgeReady,
    sceneLoadGranted,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);

  useEffect(() => {
    // `sceneLoadGranted`: the engine has finished taking the previous scene
    // off the canvas and is waiting for this one. Loading before that races
    // its despawn — see `sceneLoadGrant` above.
    if (!sceneId || !bridgeReady || !sceneLoadGranted) {
      return;
    }

    // Chrome's own copy of the previous scene's shapes, cleared for the same
    // narrowed reason as the walls loop above: the engine has already
    // despawned them from the canvas, and this keeps `worldStore` from
    // accumulating rows the canvas no longer has. Spec 002 T017's original
    // symptom — a shape from scene A still hit-testable after switching to
    // scene B — is the engine's to prevent now, and it does.
    for (const shapeId of Object.keys(worldStore.getState().shapes)) {
      worldStore.dispatch({ type: "remove_shape", shapeId }, "sync");
    }

    const shapesGeneration = sceneLoadGeneration;
    void loadShapesIntoStore(worldStore, sceneId)
      .then(() => markSceneResourceLoaded("shapes", shapesGeneration))
      .catch((error) => {
        console.error("Failed to load scene shapes:", error);
        markSceneResourceFailed("shapes", shapesGeneration);
      });

    const stopBridge = startShapeMutationBridge(worldStore, sceneId);

    return () => {
      stopBridge();
    };
  }, [
    sceneId,
    worldStore,
    bridgeReady,
    sceneLoadGranted,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);
}
