import React, { useEffect, useRef } from "react";
import { useParams } from "react-router-dom";

import { WorldWhiteboard } from "../engine/tldraw/WorldWhiteboard";
import { createWorldStore } from "../engine/world/store";
import {
  createGraphQLWorldSyncTransport,
  startWorldSync,
} from "../engine/world/sync";
import type { WorldSyncSession } from "../engine/world/sync/types";

export default function WorldView() {
  const { id = "" } = useParams();
  const loaded = useRef(false);
  const canvasId = "engine-canvas";
  const worldSyncSessionRef = useRef<WorldSyncSession | null>(null);
  const worldStoreRef = useRef(
    createWorldStore({
      worldId: id,
      initialTokens: [
        { id: "player", x: 140, y: 140, z: 0, label: "Player" },
        { id: "npc", x: 360, y: 220, z: 0, label: "NPC" },
      ],
    })
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
        worldStore: worldStoreRef.current,
        transport: createGraphQLWorldSyncTransport(),
      });
    };

    void setupSync();

    return () => {
      active = false;
      void worldSyncSessionRef.current?.stop();
      worldSyncSessionRef.current = null;
    };
  }, [id]);

  useEffect(() => {
    if (loaded.current) {
      return;
    }

    loaded.current = true;

    void import("../engine/bevy").then(({ bindWorldStore, mountEngine }) =>
      Promise.all([
        bindWorldStore(worldStoreRef.current),
        mountEngine({ canvasSelector: `#${canvasId}`, worldId: id }),
      ])
    );
  }, [canvasId, id]);

  useEffect(() => {
    if (!id) {
      return;
    }

    worldStoreRef.current.dispatch({ type: "set_world", worldId: id }, "ui");

    void import("../engine/bevy").then(({ setActiveWorld }) =>
      setActiveWorld(id)
    );
  }, [id]);

  return (
    <div data-world-id={id} style={{ width: "100vw", height: "100vh" }}>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1.2fr 1fr",
          width: "100%",
          height: "100%",
          gap: "8px",
          padding: "8px",
          boxSizing: "border-box",
        }}
      >
        <div style={{ position: "relative", width: "100%", height: "100%" }}>
          <canvas
            id={canvasId}
            style={{
              display: "block",
              width: "100%",
              height: "100%",
              borderRadius: "8px",
            }}
          />
        </div>

        <div
          style={{
            position: "relative",
            width: "100%",
            height: "100%",
            borderRadius: "8px",
            overflow: "hidden",
          }}
        >
          <WorldWhiteboard worldId={id} worldStore={worldStoreRef.current} />
        </div>
      </div>
    </div>
  );
}
