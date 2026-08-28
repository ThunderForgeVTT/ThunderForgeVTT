import { useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { Navigate, useParams } from "react-router-dom";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Loader } from "@/components/ui/loader/Loader";
import { useWorldRole } from "@/hooks/useWorldRole";
import { WorldSectionShell } from "@/layouts/world-layout/WorldSectionShell";
import { SceneDetailPage } from "@/pages/world/scenes/SceneDetailPage";
import type { SeoConfig } from "@/types/seo";
import type { WorldRecord } from "@/types/world";

/**
 * Spec 022 (FR-001a): `/world/:id/scenes/:sceneId` — the per-scene detail
 * gateway every scene now has, mirroring the existing actor/lore/item
 * detail-route convention. GM sees full management (summary, import,
 * hidden, launch); everyone else sees the read-only summary+preview.
 */
export default function SceneDetailRoutePage() {
  const { id: worldId = "", sceneId } = useParams();
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const { isGm } = useWorldRole(worldId, world);

  // Reset during render rather than at the top of the effect below: this
  // is state derived from the arguments, and doing it in the effect commits
  // one render pairing the new key with the previous key's data.
  useResetOnChange(worldId, () => {
    setIsLoading(true);
  });

  useEffect(() => {
    let active = true;

    getWorld(worldId)
      .then((result) => {
        if (active) {
          setWorld(result);
        }
      })
      .finally(() => {
        if (active) {
          setIsLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [worldId]);

  const seo: SeoConfig = {
    title: world ? `${world.name} — Scene` : "Scene",
    description: "View and manage this scene.",
    canonicalPath: `/world/${worldId}/scenes/${sceneId ?? ""}`,
    noindex: true,
  };

  if (isLoading) {
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
