import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Loader } from "@/components/ui/loader/Loader";
import { useWorldRole } from "@/hooks/useWorldRole";
import { WorldSectionShell } from "@/layouts/world-layout/WorldSectionShell";
import { PlayersPage } from "@/pages/world/players/PlayersPage";
import type { SeoConfig } from "@/types/seo";
import type { WorldRecord } from "@/types/world";

/**
 * Spec 023: `/world/:id/players` — the Players section, a dedicated routed
 * page reached from the world sidebar nav. Mirrors `ScenesRoutePage.tsx`'s
 * structure exactly.
 */
export default function PlayersRoutePage() {
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
    title: world ? `${world.name} — Players` : "World players",
    description:
      "Browse this world's members and the characters they've claimed.",
    canonicalPath: `/world/${id}/players`,
    noindex: true,
  };

  if (isLoading) {
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
