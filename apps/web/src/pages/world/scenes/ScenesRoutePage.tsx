import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Loader } from "@/components/ui/loader/Loader";
import { useWorldRole } from "@/hooks/useWorldRole";
import { WorldSectionShell } from "@/layouts/world-layout/WorldSectionShell";
import { ScenesPage } from "@/pages/world/scenes/ScenesPage";
import type { SeoConfig } from "@/types/seo";
import type { WorldRecord } from "@/types/world";

/**
 * Spec 022: `/world/:id/scenes` — the Scenes section's list view, a
 * dedicated routed page (rendered inside the normal app header/nav via
 * `MainLayout`), reached from the world sidebar nav. Mirrors
 * `WorldCompendiumRoutePage.tsx`'s structure exactly.
 */
export default function ScenesRoutePage() {
  const { id = "" } = useParams();
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const { isGm } = useWorldRole(id, world);

  useEffect(() => {
    let active = true;
    setIsLoading(true);

    getWorld(id)
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
  }, [id]);

  const seo: SeoConfig = {
    title: world ? `${world.name} — Scenes` : "World scenes",
    description: "Browse and manage this world's scenes.",
    canonicalPath: `/world/${id}/scenes`,
    noindex: true,
  };

  if (isLoading) {
    return <Loader fullScreen label="Loading scenes" />;
  }

  return (
    <>
      <SEO {...seo} />
      <WorldSectionShell worldId={id} isGm={isGm}>
        <ScenesPage worldId={id} isGm={isGm} />
      </WorldSectionShell>
    </>
  );
}
