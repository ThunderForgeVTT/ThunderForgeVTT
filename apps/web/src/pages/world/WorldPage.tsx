import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { WorldLayout } from "@/layouts/world-layout/WorldLayout";
import type { SeoConfig } from "@/types/seo";
import { WorldWhiteboard } from "@/engine/tldraw/WorldWhiteboard";
import { createWorldStore } from "@/engine/world/store";
import {
  createGraphQLWorldSyncTransport,
  loadLightsIntoStore,
  loadShapesIntoStore,
  loadTokensIntoStore,
  loadWallsIntoStore,
  startLightMutationBridge,
  startShapeMutationBridge,
  startTokenMutationBridge,
  startWallMutationBridge,
  startWorldSync,
} from "@/engine/world/sync";
import type { WorldSyncSession } from "@/engine/world/sync/types";
import { useCanvasEngine } from "@/engine/bevy/useCanvasEngine";
import { getWorld } from "@/api/world";
import { getScenes } from "@/api/scenes";
import { useAuth } from "@/hooks/useAuth";
import { WallTool } from "@/components/canvas-tools/WallTool";
import { LightingTool } from "@/components/canvas-tools/LightingTool";
import { ShapeTool } from "@/components/canvas-tools/ShapeTool";
import { MapImportTool } from "@/components/canvas-tools/MapImportTool";
import { SceneSwitcher } from "@/components/world/SceneSwitcher";
import type { WorldRecord } from "@/types/world";
import type { SceneRecord } from "@/types/scene";

export const worldPageSeo: SeoConfig = {
  title: "World workspace",
  description:
    "Launch the ThunderForge VTT collaborative world canvas powered by Bevy, tldraw, and synchronized world state.",
  canonicalPath: "/world/play",
  noindex: true,
};

export default function WorldPage() {
  const { id = "" } = useParams();
  const loaded = useRef(false);
  const canvasContainerId = "game-canvas-container";
  const worldSyncSessionRef = useRef<WorldSyncSession | null>(null);
  // Scene-scoped tokens are loaded from the server (tokens(sceneId)) once
  // a scene is selected — see the loadTokensIntoStore/startTokenMutationBridge
  // effect below — rather than seeded here with fixture data. The engine's
  // own demo "player"/"npc" tokens still spawn independently at Bevy
  // startup (src/engine/src/lib.rs) regardless of what's loaded here.
  const [worldStore] = useState(() =>
    createWorldStore({
      worldId: id,
    }),
  );
  const [worldState, setWorldState] = useState(() => worldStore.getState());
  const { user } = useAuth();
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [scenes, setScenes] = useState<SceneRecord[]>([]);
  const [selectedSceneId, setSelectedSceneId] = useState<string | null>(null);

  // GM/scene-owner check: same `createdBy === user.id` ownership
  // comparison already used to gate GM-only affordances on
  // WorldDashboardPage.tsx. Wall authoring tools (FR-009) are hidden
  // entirely for non-owners, never merely disabled.
  const isSceneOwner = Boolean(world && user && world.createdBy === user.id);
  const sceneId = selectedSceneId;
  const selectedScene = scenes.find((scene) => scene.sceneId === sceneId) ?? null;

  useEffect(() => {
    if (!id) {
      return;
    }

    let active = true;

    void getWorld(id)
      .then((response) => {
        if (active) {
          setWorld(response);
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
  }, [id]);

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
        // Default to the first scene once scenes load, but don't clobber
        // a scene the user (or a previous load) already picked.
        setSelectedSceneId((current) =>
          current && response.some((scene) => scene.sceneId === current)
            ? current
            : (response[0]?.sceneId ?? null),
        );
      })
      .catch((error) => {
        console.error("Failed to load world scenes:", error);
        if (active) {
          setScenes([]);
        }
      });

    return () => {
      active = false;
    };
  }, [id]);

  // 🎮 Phase 4.7.F1: Use canvas engine hook for responsive sizing
  const { containerRef, engineReady, error: engineError } = useCanvasEngine({
    worldId: id,
    canvasSelector: `#${canvasContainerId}`,
    onError: (err) => {
      console.error("Bevy engine failed to mount:", err);
    },
  });

  useEffect(
    () => worldStore.subscribe((event) => setWorldState(event.state)),
    [worldStore],
  );

  useEffect(() => {
    if (!id) {
      return;
    }

    let active = true;

    const setupSync = async () => {
      await worldSyncSessionRef.current?.stop();

      if (!active) {
        return;
      }

      worldSyncSessionRef.current = await startWorldSync({
        worldId: id,
        worldStore,
        transport: createGraphQLWorldSyncTransport(),
      });
    };

    void setupSync();

    return () => {
      active = false;
      void worldSyncSessionRef.current?.stop();
      worldSyncSessionRef.current = null;
    };
  }, [id, worldStore]);

  useEffect(() => {
    if (loaded.current) {
      return;
    }

    loaded.current = true;

    // Engine is now mounted via useCanvasEngine hook
    // Still need to bind world store for mutations
    void import("@/engine/bevy").then(({ bindWorldStore }) =>
      bindWorldStore(worldStore),
    );
  }, [worldStore]);

  useEffect(() => {
    if (!id) {
      return;
    }

    worldStore.dispatch({ type: "set_world", worldId: id }, "ui");

    void import("@/engine/bevy").then(({ setActiveWorld }) =>
      setActiveWorld(id),
    );
  }, [id, worldStore]);

  useEffect(() => {
    if (!selectedScene) {
      return;
    }

    // Switching scenes re-points the engine's background sprite at the
    // newly-selected scene's imported map art (null clears it for a scene
    // with no import). worldStore.dispatch's generic bindWorldStore
    // forwarder relays this to the engine the same way every other
    // WorldCommand is relayed — no direct engine-bridge call needed.
    worldStore.dispatch(
      {
        type: "set_scene_background",
        backgroundImagePath: selectedScene.backgroundImagePath,
        width: selectedScene.width,
        height: selectedScene.height,
        worldId: id,
      },
      "ui",
    );
  }, [selectedScene, worldStore, id]);

  useEffect(() => {
    if (!sceneId) {
      return;
    }

    void loadWallsIntoStore(worldStore, sceneId).catch((error) => {
      console.error("Failed to load scene walls:", error);
    });

    const stopBridge = startWallMutationBridge(worldStore, sceneId);

    return () => {
      stopBridge();
    };
  }, [sceneId, worldStore]);

  useEffect(() => {
    if (!sceneId) {
      return;
    }

    void loadTokensIntoStore(worldStore, sceneId).catch((error) => {
      console.error("Failed to load scene tokens:", error);
    });

    const stopBridge = startTokenMutationBridge(worldStore, sceneId);

    return () => {
      stopBridge();
    };
  }, [sceneId, worldStore]);

  useEffect(() => {
    if (!sceneId) {
      return;
    }

    void loadLightsIntoStore(worldStore, sceneId).catch((error) => {
      console.error("Failed to load scene lights:", error);
    });

    const stopBridge = startLightMutationBridge(worldStore, sceneId);

    return () => {
      stopBridge();
    };
  }, [sceneId, worldStore]);

  useEffect(() => {
    if (!sceneId) {
      return;
    }

    void loadShapesIntoStore(worldStore, sceneId).catch((error) => {
      console.error("Failed to load scene shapes:", error);
    });

    const stopBridge = startShapeMutationBridge(worldStore, sceneId);

    return () => {
      stopBridge();
    };
  }, [sceneId, worldStore]);

  const handleMapImportComplete = useCallback(() => {
    if (!sceneId || !id) {
      return;
    }

    // Map import creates walls/doors/lights directly in Postgres (via the
    // REST endpoint, not the GraphQL mutation bridge), so re-run the same
    // loaders used on initial mount to pull the newly imported content into
    // the world store without requiring a manual page reload.
    void loadWallsIntoStore(worldStore, sceneId).catch((error) => {
      console.error("Failed to reload scene walls after map import:", error);
    });
    void loadLightsIntoStore(worldStore, sceneId).catch((error) => {
      console.error("Failed to reload scene lights after map import:", error);
    });
    void loadShapesIntoStore(worldStore, sceneId).catch((error) => {
      console.error("Failed to reload scene shapes after map import:", error);
    });
    // Import also sets the scene's backgroundImagePath — refetch scenes so
    // the background-dispatch effect above picks up the new art.
    void getScenes(id)
      .then(setScenes)
      .catch((error) => {
        console.error("Failed to reload scenes after map import:", error);
      });
  }, [sceneId, id, worldStore]);

  const seo = useMemo<SeoConfig>(
    () => ({
      ...worldPageSeo,
      title: `${id || "World"} workspace`,
      canonicalPath: `/world/${id}/play`,
    }),
    [id],
  );

  return (
    <>
      <SEO {...seo} />
      <WorldLayout
        worldId={id}
        tokens={Object.values(worldState.tokens)}
        canvas={
          <div
            ref={containerRef}
            id={canvasContainerId}
            style={{
              display: "block",
              width: "100%",
              height: "100%",
              position: "relative",
              background: "#2a2a2a",
              overflow: "hidden",
            }}
          >
            {/* Bevy mounts canvas here */}
            {engineError && (
              <div
                style={{
                  position: "absolute",
                  top: "50%",
                  left: "50%",
                  transform: "translate(-50%, -50%)",
                  color: "red",
                  textAlign: "center",
                  zIndex: 1000,
                }}
              >
                <p>Failed to load game engine</p>
                <p style={{ fontSize: "0.9em" }}>{engineError.message}</p>
              </div>
            )}
            {scenes.length > 0 || isSceneOwner ? (
              // Not gated on scenes.length alone: a brand-new world has
              // zero scenes, and the "New scene" affordance (the only way
              // to create the first one) needs to render precisely then.
              // SceneSwitcher itself hides the scene picker when the list
              // is empty and hides "New scene" when canCreateScene is false.
              <div
                style={{
                  position: "absolute",
                  top: "1rem",
                  left: "50%",
                  transform: "translateX(-50%)",
                  // Higher than the GM tool panels (zIndex 900 below):
                  // the ShapeTool panel's row of sub-tool buttons can run
                  // wide enough to visually/pointer-overlap this
                  // top-center control at common viewport widths, and
                  // scene selection needs to stay reliably clickable
                  // regardless of which tool panel happens to paint on
                  // top of it.
                  zIndex: 950,
                  width: "14rem",
                }}
              >
                <SceneSwitcher
                  worldId={id}
                  scenes={scenes}
                  sceneId={sceneId}
                  onSceneChange={setSelectedSceneId}
                  onSceneCreated={(scene) =>
                    setScenes((current) => [...current, scene])
                  }
                  canCreateScene={isSceneOwner}
                />
              </div>
            ) : null}
            {isSceneOwner && sceneId ? (
              <div
                style={{
                  position: "absolute",
                  top: "1rem",
                  left: "1rem",
                  zIndex: 900,
                  width: "16rem",
                }}
              >
                <MapImportTool
                  // Remount on scene switch: MapImportTool tracks its last
                  // import result as local state, which otherwise persists
                  // stale across scenes (e.g. still showing "Scene One"'s
                  // import summary after switching to a scene with no
                  // imports of its own yet).
                  key={sceneId}
                  sceneId={sceneId}
                  onImportComplete={handleMapImportComplete}
                />
                <WallTool
                  worldStore={worldStore}
                  walls={worldState.walls}
                  selectedWallId={worldState.selectedWallId}
                />
                <LightingTool
                  worldStore={worldStore}
                  lights={worldState.lights}
                  selectedLightId={worldState.selectedLightId}
                  tokens={worldState.tokens}
                />
              </div>
            ) : null}
            {isSceneOwner && sceneId ? (
              <div
                style={{
                  position: "absolute",
                  top: "1rem",
                  right: "1rem",
                  zIndex: 900,
                  width: "16rem",
                }}
              >
                <ShapeTool
                  worldStore={worldStore}
                  shapes={worldState.shapes}
                  selectedShapeId={worldState.selectedShapeId}
                  sceneId={sceneId}
                  canvasContainerRef={containerRef}
                />
              </div>
            ) : null}
          </div>
        }
        whiteboard={<WorldWhiteboard worldId={id} worldStore={worldStore} />}
      />
    </>
  );
}
