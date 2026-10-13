import {
  useCallback,
  useEffect,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from "react";
import {
  loadLightsIntoStore,
  loadShapesIntoStore,
  loadWallsIntoStore,
} from "@/engine/world/sync";
import type { WorldStore } from "@/engine/world/store";
import { getScene, getScenes } from "@/api/scenes";
import type { CanvasImageAsset } from "@/api/assets";
import type { SceneRecord } from "@/types/scene";

interface UseMapImportParams {
  id: string;
  sceneId: string | null;
  worldStore: WorldStore;
  setScenes: Dispatch<SetStateAction<SceneRecord[]>>;
  setActiveSceneRecord: (scene: SceneRecord) => void;
  reloadImportedSceneRef: RefObject<(importedSceneId: string) => void>;
}

/**
 * Content that reaches the board from outside the mutation bridges: a map
 * import, and a pasted image.
 *
 * Moved out of `WorldPage` unchanged. The reload is also published through
 * the page's `reloadImportedSceneRef`, which the scene's live sync calls when
 * another client's import arrives.
 */
export function useMapImport({
  id,
  sceneId,
  worldStore,
  setScenes,
  setActiveSceneRecord,
  reloadImportedSceneRef,
}: UseMapImportParams) {
  // A map import writes walls, doors, lights and the scene's art straight to
  // Postgres (the REST endpoint, not the GraphQL mutation bridge), and says so
  // with one world event (code 13) and nothing else. So every client — the
  // importer's and everyone else's — re-runs the same loaders used on mount to
  // pull the new content in without a reload. The art reaches the board two
  // ways, both covered: the scene's levels are re-read by `useSceneLevels`
  // (the import's art is mirrored onto the entry level, which is the board's
  // art), and the scene record is re-read here for a board with no levels.
  const reloadImportedScene = useCallback(
    (importedSceneId: string) => {
      if (!id) {
        return;
      }
      void loadWallsIntoStore(worldStore, importedSceneId).catch((error) => {
        console.error("Failed to reload scene walls after map import:", error);
      });
      void loadLightsIntoStore(worldStore, importedSceneId).catch((error) => {
        console.error("Failed to reload scene lights after map import:", error);
      });
      void loadShapesIntoStore(worldStore, importedSceneId).catch((error) => {
        console.error("Failed to reload scene shapes after map import:", error);
      });
      void getScenes(id)
        .then(setScenes)
        .catch((error) => {
          console.error("Failed to reload scenes after map import:", error);
        });
      // A player's list leaves out a hidden scene — and a world's own
      // Starting Scene is hidden — so the record they play from is the one
      // fetched by id, and that is the one that has to be fetched again.
      void getScene(importedSceneId)
        .then((scene) => {
          if (scene) {
            setActiveSceneRecord(scene);
          }
        })
        .catch((error) => {
          console.error("Failed to reload the scene after map import:", error);
        });
    },
    // `setActiveSceneRecord` is a state setter from `useSceneRecord`: stable.
    // So is `setScenes`, the page's own.
    [id, worldStore, setActiveSceneRecord, setScenes],
  );
  useEffect(() => {
    reloadImportedSceneRef.current = reloadImportedScene;
    // The ref is the page's `useRef`: stable.
  }, [reloadImportedScene, reloadImportedSceneRef]);

  const handleMapImportComplete = useCallback(() => {
    if (!sceneId) {
      return;
    }
    // The importer's own tab does not wait for the event to come back round.
    reloadImportedScene(sceneId);
  }, [sceneId, reloadImportedScene]);

  // T023/T030 (specs/002-canvas-authoring-asset-storage): a pasted image
  // is already persisted by the time AssetPasteTool calls this (the
  // GraphQL mutation already succeeded) — this just tells the engine to
  // spawn it. `path` points at the authenticated `/canvas-assets/{id}`
  // proxy route (crates/thunderforge-server/src/canvas_assets_serve.rs), not RustFS
  // directly: RustFS is private per-campaign storage, so the browser can
  // never fetch from it directly — this proxy is what makes a pasted
  // image (or, latently, a migrated background) actually renderable at
  // all, a gap that existed silently until this was wired up (nothing
  // previously exercised the read path end-to-end).
  const handleAssetPasted = useCallback(
    (asset: CanvasImageAsset) => {
      worldStore.dispatch(
        {
          type: "upsert_canvas_image_asset",
          assetId: asset.id,
          // `.webp`, not a bare id: the engine resolves an image
          // loader by extension (see canvas_assets_serve::parse_asset_id).
          path: `/api/canvas-assets/${asset.id}.webp`,
          x: 0,
          y: 0,
          width: asset.widthPx,
          height: asset.heightPx,
        },
        "ui",
      );
    },
    [worldStore],
  );

  return { handleMapImportComplete, handleAssetPasted };
}
