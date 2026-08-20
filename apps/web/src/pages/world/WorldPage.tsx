import { useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { WorldLayout } from "@/layouts/world-layout/WorldLayout";
import type { SeoConfig } from "@/types/seo";
import { WorldWhiteboard } from "@/engine/tldraw/WorldWhiteboard";
import { createWorldStore } from "@/engine/world/store";
import {
  createGraphQLWorldSyncTransport,
  startWorldSync,
} from "@/engine/world/sync";
import type { WorldSyncSession } from "@/engine/world/sync/types";
import { useCanvasEngine } from "@/engine/bevy/useCanvasEngine";

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
  const [worldStore] = useState(() =>
    createWorldStore({
      worldId: id,
      initialTokens: [
        { id: "player", x: 140, y: 140, z: 0, label: "Player" },
        { id: "npc", x: 360, y: 220, z: 0, label: "NPC" },
      ],
    }),
  );
  const [worldState, setWorldState] = useState(() => worldStore.getState());

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
          </div>
        }
        whiteboard={<WorldWhiteboard worldId={id} worldStore={worldStore} />}
      />
    </>
  );
}
