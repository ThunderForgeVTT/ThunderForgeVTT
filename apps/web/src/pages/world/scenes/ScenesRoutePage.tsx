import { useParams } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Loader } from "@/components/ui/loader/Loader";
import { useKnownWorld } from "@/hooks/world-chrome-context";
import { useWorldRole } from "@/hooks/useWorldRole";
import { WorldSectionShell } from "@/layouts/world-layout/WorldSectionShell";
import { ScenesPage } from "@/pages/world/scenes/ScenesPage";
import type { SeoConfig } from "@/types/seo";

/**
 * Spec 022: `/world/:id/scenes` — the Scenes section's list view, a
 * dedicated routed page (rendered inside the normal app header/nav via
 * `MainLayout`), reached from the world sidebar nav. Mirrors
 * `WorldCompendiumRoutePage.tsx`'s structure exactly.
 */
export default function ScenesRoutePage() {
  const { id = "" } = useParams();
  // Spec 101 FR-001: the world as last known, so moving between its pages
  // keeps the page on screen; only a first visit waits.
  const { world, known } = useKnownWorld(id);
  const { isGm } = useWorldRole(id, world);

  const seo: SeoConfig = {
    title: world ? `${world.name} — Scenes` : "World scenes",
    description: "Browse and manage this world's scenes.",
    canonicalPath: `/world/${id}/scenes`,
    noindex: true,
  };

  if (!known) {
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
