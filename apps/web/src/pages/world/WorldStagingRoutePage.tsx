import { useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { useParams } from "react-router-dom";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Loader } from "@/components/ui/loader/Loader";
import { useAuth } from "@/hooks/useAuth";
import { useWorldRole } from "@/hooks/useWorldRole";
import { WorldSectionShell } from "@/layouts/world-layout/WorldSectionShell";
import { WorldStagingPage } from "@/layouts/world-layout/WorldStagingPage";
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
 *
 * Spec 022 (FR-002): no longer fetches/owns scenes — scene selection now
 * happens exclusively via the Scenes section's Launch action.
 */
export default function WorldStagingRoutePage() {
  const { id = "" } = useParams();
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const { isGm, loading: roleLoading } = useWorldRole(id, world);
  const { user } = useAuth();

  // Reset during render rather than at the top of the effect below: this
  // is state derived from the arguments, and doing it in the effect commits
  // one render pairing the new key with the previous key's data.
  useResetOnChange(id, () => {
    setIsLoading(true);
  });

  useEffect(() => {
    let active = true;

    getWorld(id)
      .then((worldResult) => {
        if (active) {
          setWorld(worldResult);
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
      <WorldSectionShell worldId={id} isGm={isGm}>
        <WorldStagingPage
          worldId={id}
          world={world}
          isGm={isGm}
          currentUserId={user?.id}
          onSessionNotesSaved={(notes) =>
            setWorld((current) =>
              current ? { ...current, sessionNotes: notes } : current,
            )
          }
        />
      </WorldSectionShell>
    </>
  );
}
