import { useParams } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Loader } from "@/components/ui/loader/Loader";
import { useKnownWorld } from "@/hooks/world-chrome-context";
import { useWorldRole } from "@/hooks/useWorldRole";
import { WorldSectionShell } from "@/layouts/world-layout/WorldSectionShell";
import { PlayersPage } from "@/pages/world/players/PlayersPage";
import type { SeoConfig } from "@/types/seo";

/**
 * Spec 023: `/world/:id/players` — the Players section, a dedicated routed
 * page reached from the world sidebar nav. Mirrors `ScenesRoutePage.tsx`'s
 * structure exactly.
 */
export default function PlayersRoutePage() {
  const { id = "" } = useParams();
  // Spec 101 FR-001: the world as last known, so moving between its pages
  // keeps the page on screen; only a first visit waits.
  const { world, known } = useKnownWorld(id);
  const { isGm } = useWorldRole(id, world);

  const seo: SeoConfig = {
    title: world ? `${world.name} — Players` : "World players",
    description:
      "Browse this world's members and the characters they've claimed.",
    canonicalPath: `/world/${id}/players`,
    noindex: true,
  };

  if (!known) {
    return <Loader fullScreen label="Loading players" />;
  }

  return (
    <>
      <SEO {...seo} />
      <WorldSectionShell worldId={id} isGm={isGm}>
        <PlayersPage worldId={id} isGm={isGm} />
      </WorldSectionShell>
    </>
  );
}
