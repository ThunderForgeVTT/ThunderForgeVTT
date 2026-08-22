import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Loader } from "@/components/ui/loader/Loader";
import { WorldCompendiumPage } from "@/pages/world/compendium/WorldCompendiumPage";
import type { SeoConfig } from "@/types/seo";
import type { WorldRecord } from "@/types/world";

/**
 * Spec 011: `/world/:id/compendium` — a dedicated, routed page (rendered
 * inside the normal app header/nav via `MainLayout`), reached from
 * Session Setup's Compendium link. Never mounts the canvas, following the
 * exact precedent of `WorldStagingRoutePage.tsx` (spec 010).
 */
export default function WorldCompendiumRoutePage() {
  const { id = "" } = useParams();
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [isLoading, setIsLoading] = useState(true);

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
    title: world ? `${world.name} — Compendium` : "World compendium",
    description: "Browse and curate this world's NPCs, items, and abilities.",
    canonicalPath: `/world/${id}/compendium`,
    noindex: true,
  };

  if (isLoading) {
    return <Loader fullScreen label="Loading compendium" />;
  }

  return (
    <>
      <SEO {...seo} />
      <WorldCompendiumPage worldId={id} world={world} />
    </>
  );
}
