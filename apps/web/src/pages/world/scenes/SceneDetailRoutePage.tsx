import { Navigate, useParams } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Loader } from "@/components/ui/loader/Loader";
import { useKnownWorld } from "@/hooks/world-chrome-context";
import { useWorldRole } from "@/hooks/useWorldRole";
import { WorldSectionShell } from "@/layouts/world-layout/WorldSectionShell";
import { SceneDetailPage } from "@/pages/world/scenes/SceneDetailPage";
import type { SeoConfig } from "@/types/seo";

/**
 * Spec 022 (FR-001a): `/world/:id/scenes/:sceneId` — the per-scene detail
 * gateway every scene now has, mirroring the existing actor/lore/item
 * detail-route convention. GM sees full management (summary, import,
 * hidden, launch); everyone else sees the read-only summary+preview.
 */
export default function SceneDetailRoutePage() {
  const { id: worldId = "", sceneId } = useParams();
  // Spec 101 FR-001: the world as last known, so moving between its pages
  // keeps the page on screen; only a first visit waits.
  const { world, known } = useKnownWorld(worldId);
  const { isGm } = useWorldRole(worldId, world);

  const seo: SeoConfig = {
    title: world ? `${world.name} — Scene` : "Scene",
    description: "View and manage this scene.",
    canonicalPath: `/world/${worldId}/scenes/${sceneId ?? ""}`,
    noindex: true,
  };

  if (!known) {
    return <Loader fullScreen label="Loading scene" />;
  }

  if (!sceneId) {
    return <Navigate to={`/world/${worldId}/scenes`} replace />;
  }

  return (
    <>
      <SEO {...seo} />
      <WorldSectionShell worldId={worldId} isGm={isGm}>
        <SceneDetailPage worldId={worldId} sceneId={sceneId} isGm={isGm} />
      </WorldSectionShell>
    </>
  );
}
