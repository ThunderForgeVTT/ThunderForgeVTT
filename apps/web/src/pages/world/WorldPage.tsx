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

export const worldPageSeo: SeoConfig = {
  title: "World workspace",
  description:
    "Launch the ThunderForge VTT collaborative world canvas powered by Bevy, tldraw, and synchronized world state.",
  canonicalPath: "/world",
  noindex: true,
};

export default function WorldPage() {
  const { id = "" } = useParams();
  const loaded = useRef(false);
  const canvasId = "engine-canvas";
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

  useEffect(() => worldStore.subscribe((event) => setWorldState(event.state)), [worldStore]);

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

    void import("@/engine/bevy").then(({ bindWorldStore, mountEngine }) =>
      Promise.all([
        bindWorldStore(worldStore),
        mountEngine({ canvasSelector: `#${canvasId}`, worldId: id }),
      ]),
    );
  }, [canvasId, id, worldStore]);

  useEffect(() => {
    if (!id) {
      return;
    }

    worldStore.dispatch({ type: "set_world", worldId: id }, "ui");

    void import("@/engine/bevy").then(({ setActiveWorld }) => setActiveWorld(id));
  }, [id, worldStore]);

  const seo = useMemo<SeoConfig>(
    () => ({
      ...worldPageSeo,
      title: `${id || "World"} workspace`,
      canonicalPath: `/world/${id}`,
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
          <canvas
            id={canvasId}
            style={{ display: "block", width: "100%", height: "100%" }}
          />
        }
        whiteboard={
          <WorldWhiteboard worldId={id} worldStore={worldStore} />
        }
      />
    </>
  );
}
