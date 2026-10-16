import { useParams } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Loader } from "@/components/ui/loader/Loader";
import { useKnownWorld } from "@/hooks/world-chrome-context";
import { useWorldRole } from "@/hooks/useWorldRole";
import { WorldSectionShell } from "@/layouts/world-layout/WorldSectionShell";
import { WorldCompendiumPage } from "@/pages/world/compendium/WorldCompendiumPage";
import type { SeoConfig } from "@/types/seo";

/**
 * Spec 011: `/world/:id/compendium` — a dedicated, routed page (rendered
 * inside the normal app header/nav via `MainLayout`), reached from
 * Session Setup's Compendium link. Never mounts the canvas, following the
 * exact precedent of `WorldStagingRoutePage.tsx` (spec 010).
 */
export default function WorldCompendiumRoutePage() {
  const { id = "" } = useParams();
  // Spec 101 FR-001: the world as last known, so moving between its pages
  // keeps the page on screen; only a first visit waits.
  const { world, known } = useKnownWorld(id);
  const { isGm } = useWorldRole(id, world);

  const seo: SeoConfig = {
    title: world ? `${world.name} — Compendium` : "World compendium",
    description: "Browse and curate this world's NPCs, items, and abilities.",
    canonicalPath: `/world/${id}/compendium`,
    noindex: true,
  };

  if (!known) {
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
