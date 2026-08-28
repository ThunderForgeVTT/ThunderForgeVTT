import { useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { useParams } from "react-router-dom";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Loader } from "@/components/ui/loader/Loader";
import { useWorldRole } from "@/hooks/useWorldRole";
import { WorldSectionShell } from "@/layouts/world-layout/WorldSectionShell";
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
  const { isGm } = useWorldRole(id, world);

  // Reset during render rather than at the top of the effect below: this
  // is state derived from the arguments, and doing it in the effect commits
  // one render pairing the new key with the previous key's data.
  useResetOnChange(id, () => {
    setIsLoading(true);
  });

  useEffect(() => {
    let active = true;

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
      <WorldSectionShell worldId={id} isGm={isGm}>
        <WorldCompendiumPage worldId={id} world={world} />
      </WorldSectionShell>
    </>
  );
}
