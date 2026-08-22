import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { WorldLayout } from "@/layouts/world-layout/WorldLayout";
import { WorldStagingPage } from "@/layouts/world-layout/WorldStagingPage";
import type { SeoConfig } from "@/types/seo";
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
import { useWorldRole } from "@/hooks/useWorldRole";
import { WallTool } from "@/components/canvas-tools/WallTool";
import { LightingTool } from "@/components/canvas-tools/LightingTool";
import { ShapeTool } from "@/components/canvas-tools/ShapeTool";
import { MapImportTool } from "@/components/canvas-tools/MapImportTool";
import { AssetPasteTool } from "@/components/canvas-tools/AssetPasteTool";
import { TokenTool } from "@/components/canvas-tools/TokenTool";
import { TokenPanel } from "@/components/TokenPanel";
import type { CanvasImageAsset } from "@/api/assets";
import type { WorldRecord } from "@/types/world";
import type { SceneRecord } from "@/types/scene";

export const worldPageSeo: SeoConfig = {
  title: "World workspace",
  description:
    "Launch the ThunderForge VTT collaborative world canvas powered by Bevy and synchronized world state.",
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
  // True once `bindWorldStore` has actually finished registering its
  // store subscription (the thing that forwards "sync"-sourced dispatches
  // — e.g. loadWallsIntoStore's confirmed wall rows — into the engine via
  // apply_world_command). NOT the same as `engineReady`: bindWorldStore is
  // reached through its own `import("@/engine/bevy")` dynamic-chunk hop,
  // which `useCanvasEngine`'s static-imported `mountEngine` doesn't pay,
  // so `engineReady` can and does flip true before this subscription
  // exists. Without gating on this specifically (found via
  // specs/002-canvas-authoring-asset-storage T014's live debugging),
  // walls/tokens/lights/shapes fetched on mount/reload can lose the race:
  // the GraphQL fetch resolves and dispatches "upsert_wall" (etc.) before
  // anyone is listening to relay it into the engine's WallSet/TokenSet/
  // etc. — the dispatch still updates React state correctly, so nothing
  // looks broken in the UI's data, but the engine-side resource silently
  // stays empty (e.g. no wall is selectable-by-click after a reload, even
  // though it's genuinely persisted and the property panel would show it
  // if selection worked). Every effect that dispatches something meant to
  // reach the engine gates on this now.
  const [bridgeReady, setBridgeReady] = useState(false);
  // T006/T009: TokenPanel is mounted for both GM and player (a player needs
  // it to edit their own primary token's photo — FR-009a) via a toggle
  // button always visible once a scene is selected; the panel's own
  // internal isSceneOwner check (see TokenPanel.tsx) restricts create/
  // delete/ownership-assignment controls to the GM.
  const [tokenPanelOpen, setTokenPanelOpen] = useState(false);

  // GM/scene-owner check. Spec 009 (T007): this used to be a bare
  // `world.createdBy === user.id` comparison, which only recognized the
  // world's original creator — an invited co-GM (a `world_members` row
  // with role "GM") was silently treated as a non-owner. `useWorldRole`
  // fixes that by also matching an accepted GM-role membership, falling
  // back to the same createdBy check when no membership row exists yet
  // (research.md §3). Wall authoring tools (FR-009) are hidden entirely
  // for non-GMs, never merely disabled.
  const { isGm: isSceneOwner } = useWorldRole(id, world);
  // Spec 009 (T010): staging page vs full-screen canvas is a local,
  // per-user UI state — never synced across users (FR-014) and never
  // reflected in the URL. The canvas container below is rendered
  // unconditionally regardless of this value (only its CSS visibility
  // changes) so the already-booted Bevy engine's canvas handle never goes
  // stale (research.md §1).
  const [playView, setPlayView] = useState<"staging" | "playing">("staging");
  const sceneId = selectedSceneId;

  // Spec 004 (US4, T034-T036): scene-switch loading/error feedback. Before
  // this, the four per-scene loaders below (walls/tokens/lights/shapes)
  // plus the background-image dispatch had zero UI signal on failure —
  // just a `console.error`, per contracts/scene-load-state.md's
  // documented gap. `sceneLoadGeneration` is the "retry"/"latest wins"
  // mechanism (T036): bumping it both re-triggers every loader effect
  // below (added to each one's dependency array) and invalidates any
  // in-flight response from a stale scene/attempt via
  // `sceneLoadGenerationRef` — a loader that resolves after the user has
  // already switched scenes or hit retry again must not resurrect a
  // finished "loading" state or overwrite a newer error.
  const [sceneLoadGeneration, setSceneLoadGeneration] = useState(0);
  const sceneLoadGenerationRef = useRef(0);
  useEffect(() => {
    sceneLoadGenerationRef.current = sceneLoadGeneration;
  }, [sceneLoadGeneration]);

  type SceneLoadResource = "background" | "walls" | "tokens" | "lights" | "shapes";
  type SceneLoadState =
    | { status: "loading" }
    | { status: "ready" }
    | { status: "error"; failedResource: SceneLoadResource };

  const [sceneLoadState, setSceneLoadState] = useState<SceneLoadState>({ status: "loading" });
  const pendingSceneResourcesRef = useRef<Set<SceneLoadResource>>(new Set());

  // Reset to "loading" whenever the active scene or the retry generation
  // changes — mirrors the walls/shapes "clear the previous scene's stale
  // data" effects below, but for load-status rather than store contents.
  useEffect(() => {
    if (!sceneId) {
      return;
    }
    pendingSceneResourcesRef.current = new Set<SceneLoadResource>([
      "background",
      "walls",
      "tokens",
      "lights",
      "shapes",
    ]);
    setSceneLoadState({ status: "loading" });
  }, [sceneId, sceneLoadGeneration]);

  const markSceneResourceLoaded = useCallback(
    (resource: SceneLoadResource, generation: number) => {
      if (generation !== sceneLoadGenerationRef.current) {
        return;
      }
      pendingSceneResourcesRef.current.delete(resource);
      if (pendingSceneResourcesRef.current.size === 0) {
        setSceneLoadState((current) =>
          current.status === "error" ? current : { status: "ready" },
        );
      }
    },
    [],
  );

  const markSceneResourceFailed = useCallback(
    (resource: SceneLoadResource, generation: number) => {
      if (generation !== sceneLoadGenerationRef.current) {
        return;
      }
      // Background-image failure takes priority in the reported
      // `failedResource` if multiple fail simultaneously, per
      // contracts/scene-load-state.md — it's the most visually
      // disruptive, so don't let a later walls/lights/shapes/tokens
      // failure downgrade an already-reported background failure.
      setSceneLoadState((current) => {
        if (current.status === "error" && current.failedResource === "background") {
          return current;
        }
        return { status: "error", failedResource: resource };
      });
    },
    [],
  );

  const retrySceneLoad = useCallback(() => {
    setSceneLoadGeneration((generation) => generation + 1);
  }, []);
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

  // Spec 009: stabilized with useCallback — useCanvasEngine's mount effect
  // depends on this callback's identity (see useCanvasEngine.ts), so a
  // fresh inline function here re-triggers mountEngine() on every render.
  // That was always latent, but WorldPage.tsx didn't previously have a
  // render-triggering subscription (useWorldRole -> useWorldMembers, added
  // in this spec) frequent enough to actually surface it as an infinite
  // "Maximum update depth exceeded" loop.
  const handleEngineError = useCallback((err: Error) => {
    console.error("Bevy engine failed to mount:", err);
  }, []);

  // 🎮 Phase 4.7.F1: Use canvas engine hook for responsive sizing
  const { containerRef, engineReady, loadStage, error: engineError } = useCanvasEngine({
    worldId: id,
    canvasSelector: `#${canvasContainerId}`,
    onError: handleEngineError,
  });

  // Spec 009: Bevy/winit inserts the real <canvas> as a direct child of
  // <body> (verified via live inspection), not as a descendant of
  // `#game-canvas-container` — the container div only reserves layout
  // space for it, it never actually contains it. Two consequences this
  // effect handles directly, since neither can be solved with CSS on our
  // own React tree:
  // 1. `display:none` on any wrapper around the container never hides the
  //    real canvas — it must be toggled on the canvas element itself.
  // 2. With no positioning of its own, the canvas sits in normal document
  //    flow as the last child of <body> — i.e. *after* the full-viewport
  //    `WorldLayout`/`WorldStagingPage` wrappers that come before it in the
  //    DOM, landing one viewport-height below where it's meant to render.
  //    `position: fixed; inset: 0` removes it from that flow entirely so
  //    it always fills the viewport regardless of surrounding page content.
  // A plain effect keyed on [playView] can miss the exact moment the
  // canvas is inserted (e.g. right after a reload, mid-WASM-boot) and
  // then never re-fire since nothing else changes — a MutationObserver on
  // <body>, plus applying playViewRef's current value whenever a canvas
  // shows up, catches it regardless of timing.
  const playViewRef = useRef(playView);
  playViewRef.current = playView;

  const applyCanvasVisibility = useCallback((canvas: HTMLCanvasElement) => {
    if (playViewRef.current === "playing") {
      canvas.style.display = "";
      canvas.style.position = "fixed";
      canvas.style.inset = "0";
    } else {
      canvas.style.display = "none";
    }
  }, []);

  useEffect(() => {
    const existing = document.querySelector<HTMLCanvasElement>("canvas");
    if (existing) {
      applyCanvasVisibility(existing);
    }

    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        mutation.addedNodes.forEach((node) => {
          if (node instanceof HTMLCanvasElement) {
            applyCanvasVisibility(node);
          }
        });
      }
    });
    observer.observe(document.body, { childList: true });

    return () => observer.disconnect();
  }, [applyCanvasVisibility]);

  useEffect(() => {
    const canvas = document.querySelector<HTMLCanvasElement>("canvas");
    if (canvas) {
      applyCanvasVisibility(canvas);
    }
  }, [playView, engineReady, applyCanvasVisibility]);

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
      bindWorldStore(worldStore).then(() => setBridgeReady(true)),
    );
  }, [worldStore]);

  // FR-010: tell the engine whether this session may author walls/shapes.
  // Without this, WallPlugin/ShapePlugin's IsGameMaster resource stays at
  // its `false` default forever and no click/keyboard authoring input is
  // ever accepted, regardless of `isSceneOwner`. Re-sent whenever
  // ownership resolves/changes or the engine (re)becomes ready, since a
  // fresh `start()` always resets the wasm side back to the default.
  useEffect(() => {
    if (!engineReady) {
      return;
    }

    void import("@/engine/bevy").then(({ setIsGameMaster }) =>
      setIsGameMaster(isSceneOwner),
    );
  }, [engineReady, isSceneOwner]);

  useEffect(() => {
    if (!id || !bridgeReady) {
      return;
    }

    worldStore.dispatch({ type: "set_world", worldId: id }, "ui");

    void import("@/engine/bevy").then(({ setActiveWorld }) =>
      setActiveWorld(id),
    );
  }, [id, worldStore, bridgeReady]);

  useEffect(() => {
    if (!selectedScene || !bridgeReady) {
      return;
    }

    // Switching scenes re-points the engine's background sprite at the
    // newly-selected scene's imported map art (null clears it for a scene
    // with no import). worldStore.dispatch's generic bindWorldStore
    // forwarder relays this to the engine the same way every other
    // WorldCommand is relayed — no direct engine-bridge call needed.
    // Gated on `bridgeReady` (see its declaration) so this dispatch isn't
    // lost to the bindWorldStore-registration race.
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

    // Spec 004 (US4): a scene with no imported art has nothing to fail —
    // mark "background" satisfied immediately. A scene with art gets a
    // real reachability check (the engine loads the sprite itself with no
    // JS-visible success/failure signal, so this is the only way to
    // surface "the background asset is unreachable" per FR-013 rather
    // than leaving the canvas silently blank).
    const generation = sceneLoadGeneration;
    if (!selectedScene.backgroundImagePath) {
      markSceneResourceLoaded("background", generation);
      return;
    }

    let cancelled = false;
    void fetch(selectedScene.backgroundImagePath, {
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
    worldStore,
    id,
    bridgeReady,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);

  useEffect(() => {
    if (!sceneId || !bridgeReady) {
      return;
    }

    // Switching scenes must clear the *previous* scene's walls first:
    // loadWallsIntoStore only ever upserts, so without this, a
    // previously-visited scene's walls (and a stale selection pointing
    // at one of them) stay rendered/selectable on top of whichever scene
    // is now active, indefinitely accumulating across scene switches for
    // the life of the session (found while investigating spec 002 T017 —
    // the equivalent gap for shapes below reproduced as a real, visible
    // bug, not just a stale-selection artifact: an old scene's shape was
    // still spawned and hit-testable in the engine after switching
    // scenes). `"sync"` source: a confirmed-state correction, not a
    // delete intent — startWallMutationBridge below ignores
    // `source: "sync"` dispatches, so this never calls deleteWall.
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
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);

  useEffect(() => {
    if (!sceneId || !bridgeReady) {
      return;
    }

    const tokensGeneration = sceneLoadGeneration;
    void loadTokensIntoStore(worldStore, sceneId)
      .then(() => markSceneResourceLoaded("tokens", tokensGeneration))
      .catch((error) => {
        console.error("Failed to load scene tokens:", error);
        markSceneResourceFailed("tokens", tokensGeneration);
      });

    const stopBridge = startTokenMutationBridge(worldStore, sceneId, isSceneOwner);

    return () => {
      stopBridge();
    };
  }, [
    sceneId,
    worldStore,
    bridgeReady,
    isSceneOwner,
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);

  useEffect(() => {
    if (!sceneId || !bridgeReady) {
      return;
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
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);

  useEffect(() => {
    if (!sceneId || !bridgeReady) {
      return;
    }

    // Clear the previous scene's shapes before loading this scene's —
    // see the matching comment on the walls effect above for why. This
    // is the fix for the real bug behind spec 002 T017's failure: without
    // it, a shape drawn on scene A stayed spawned (and hit-testable) in
    // the engine after switching to scene B, so a click on B's empty
    // canvas at the same on-screen position re-selected A's shape.
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
    sceneLoadGeneration,
    markSceneResourceLoaded,
    markSceneResourceFailed,
  ]);

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

  // T023/T030 (specs/002-canvas-authoring-asset-storage): a pasted image
  // is already persisted by the time AssetPasteTool calls this (the
  // GraphQL mutation already succeeded) — this just tells the engine to
  // spawn it. `path` points at the authenticated `/canvas-assets/{id}`
  // proxy route (src/server/src/canvas_assets_serve.rs), not RustFS
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
          path: `/api/canvas-assets/${asset.id}`,
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
      <div style={{ display: playView === "staging" ? "block" : "none" }}>
        <WorldStagingPage
          worldId={id}
          world={world}
          scenes={scenes}
          sceneId={sceneId}
          onSceneChange={setSelectedSceneId}
          onSceneCreated={(scene) =>
            setScenes((current) => [...current, scene])
          }
          isGm={isSceneOwner}
          onPlay={() => setPlayView("playing")}
        />
      </div>
      <div style={{ display: playView === "playing" ? "block" : "none" }}>
      <WorldLayout
        onBackToStaging={() => setPlayView("staging")}
        worldId={id}
        tokens={Object.values(worldState.tokens)}
        scenes={scenes}
        sceneId={sceneId}
        onSceneChange={setSelectedSceneId}
        onSceneCreated={(scene) =>
          setScenes((current) => [...current, scene])
        }
        isGm={isSceneOwner}
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
            {/* Spec 008 (US1, FR-002/SC-002): the flat #2a2a2a background
             * above used to show nothing at all here while the ~190MB
             * WASM engine loaded — this closes that gap with continuous
             * status text, styled identically to the scene-load-indicator
             * block below it. */}
            {!engineReady && !engineError ? (
              <div
                data-testid="engine-load-indicator"
                style={{
                  position: "absolute",
                  top: "50%",
                  left: "50%",
                  transform: "translate(-50%, -50%)",
                  color: "white",
                  textAlign: "center",
                  zIndex: 1000,
                }}
              >
                <p>
                  {loadStage === "downloading"
                    ? "Downloading engine…"
                    : "Starting engine…"}
                </p>
              </div>
            ) : null}
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
            {sceneId && sceneLoadState.status === "loading" ? (
              <div
                data-testid="scene-load-indicator"
                style={{
                  position: "absolute",
                  top: "50%",
                  left: "50%",
                  transform: "translate(-50%, -50%)",
                  color: "white",
                  textAlign: "center",
                  zIndex: 1000,
                }}
              >
                <p>Loading scene…</p>
              </div>
            ) : null}
            {sceneId && sceneLoadState.status === "error" ? (
              <div
                data-testid="scene-load-error"
                style={{
                  position: "absolute",
                  top: "50%",
                  left: "50%",
                  transform: "translate(-50%, -50%)",
                  color: "white",
                  textAlign: "center",
                  zIndex: 1000,
                }}
              >
                <p>
                  Failed to load{" "}
                  {sceneLoadState.failedResource === "background"
                    ? "the scene's background image"
                    : `the scene's ${sceneLoadState.failedResource}`}
                  .
                </p>
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  data-testid="scene-load-retry"
                  onClick={retrySceneLoad}
                >
                  Retry
                </Button>
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
                <TokenTool
                  worldStore={worldStore}
                  tokens={worldState.tokens}
                  selectedTokenId={worldState.selectedTokenId}
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
            {isSceneOwner && sceneId ? (
              <AssetPasteTool
                worldId={id}
                sceneId={sceneId}
                active={true}
                onPasted={handleAssetPasted}
              />
            ) : null}
            {sceneId ? (
              <div
                style={{
                  position: "absolute",
                  bottom: "1rem",
                  right: "1rem",
                  zIndex: 900,
                }}
              >
                <button
                  type="button"
                  data-testid="token-panel-toggle-button"
                  onClick={() => setTokenPanelOpen(true)}
                  className="token-panel-toggle-button"
                >
                  Tokens
                </button>
                <TokenPanel
                  sceneId={sceneId}
                  currentUserId={user?.id ?? null}
                  isSceneOwner={isSceneOwner}
                  isOpen={tokenPanelOpen}
                  onOpenChange={setTokenPanelOpen}
                />
              </div>
            ) : null}
          </div>
        }
      />
      </div>
    </>
  );
}
