import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { getScenes } from "@/api/scenes";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Loader } from "@/components/ui/loader/Loader";
import { useWorldRole } from "@/hooks/useWorldRole";
import { WorldStagingPage } from "@/layouts/world-layout/WorldStagingPage";
import type { SceneRecord } from "@/types/scene";
import type { SeoConfig } from "@/types/seo";
import type { WorldRecord } from "@/types/world";

/**
 * Spec 010: `/world/:id/staging` — a dedicated, routed staging screen
 * (rendered inside the normal app header/nav via `MainLayout`), reached
 * from `/welcome`'s "Enter" link. Replaces spec 009's approach of treating
 * staging as a client-side UI state inside `/world/:id/play`; the canvas
 * container is never mounted here, so there is no risk to the Bevy
 * engine's canvas handle (research.md §1) — `/play` itself is now
 * canvas-only (see `WorldPage.tsx`).
 */
export default function WorldStagingRoutePage() {
  const { id = "" } = useParams();
  const navigate = useNavigate();
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [scenes, setScenes] = useState<SceneRecord[]>([]);
  const [sceneId, setSceneId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const { isGm, loading: roleLoading } = useWorldRole(id, world);

  useEffect(() => {
    let active = true;
    setIsLoading(true);

    Promise.all([getWorld(id), getScenes(id)])
      .then(([worldResult, scenesResult]) => {
        if (!active) {
          return;
        }
        setWorld(worldResult);
        setScenes(scenesResult);
        setSceneId((current) => current ?? scenesResult[0]?.sceneId ?? null);
      })
      .finally(() => {
        if (active) {
          setIsLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [id]);

  const seo: SeoConfig = {
    title: world ? `${world.name} — Staging` : "World staging",
    description: "Catalog actors, review the roster, and start the session.",
    canonicalPath: `/world/${id}/staging`,
    noindex: true,
  };

  if (isLoading || roleLoading) {
    return <Loader fullScreen label="Loading world staging" />;
  }

  return (
    <>
      <SEO {...seo} />
      <WorldStagingPage
        worldId={id}
        world={world}
        scenes={scenes}
        sceneId={sceneId}
        onSceneChange={setSceneId}
        onSceneCreated={(scene) => setScenes((current) => [...current, scene])}
        isGm={isGm}
        onPlay={() => navigate(`/world/${id}/play`)}
        onSessionNotesSaved={(notes) =>
          setWorld((current) => (current ? { ...current, sessionNotes: notes } : current))
        }
      />
    </>
  );
}
