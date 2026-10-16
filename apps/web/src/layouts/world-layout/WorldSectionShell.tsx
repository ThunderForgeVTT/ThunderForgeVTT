import { useEffect, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { getResumeSceneName } from "@/api/worldStatistics";
import { Button } from "@/components/ui/button/Button";
import { WorldAppearance } from "@/appearance/WorldAppearance";
import { PlayPausedBanner } from "@/components/world/PlayPausedBanner";
import { WorldSidebarNav } from "@/layouts/world-layout/WorldSidebarNav";

export interface WorldSectionShellProps {
  worldId: string;
  isGm: boolean;
  children: ReactNode;
}

/**
 * Shared chrome for the world's "hub" pages (Session Setup, Compendium,
 * System settings) — a persistent category sidebar plus a consistent
 * top-right Play button, alongside whatever the page itself renders.
 * Kept as a thin wrapper (not a nested route layout) so each route page's
 * existing data-fetching is untouched. Play lives here (rather than in the
 * sidebar's category list or duplicated per-page) so every screen gets
 * exactly one, in the same place.
 *
 * Play names the scene it resumes in (spec 100 US3.4) on every page here,
 * not only the overview: the shell asks once per world, and says "Play"
 * until it knows, or when no scene is active.
 *
 * Width and gutter match `Container` (spec 100 US4): `--page-max` caps the
 * page on ultrawide screens, `w-full` lets it shrink freely below that. Play's own full-screen canvas is unrelated
 * chrome and isn't wrapped by this component.
 */
export function WorldSectionShell({
  worldId,
  isGm,
  children,
}: WorldSectionShellProps) {
  const [resume, setResume] = useState<{
    worldId: string;
    scene: string | null;
  } | null>(null);
  useEffect(() => {
    let active = true;
    getResumeSceneName(worldId)
      .then((scene) => {
        if (active) setResume({ worldId, scene });
      })
      .catch(() => {
        // Play still works; it just does not name the scene.
      });
    return () => {
      active = false;
    };
  }, [worldId]);
  const scene = resume?.worldId === worldId ? resume.scene : null;

  return (
    <WorldAppearance worldId={worldId}>
      <div
        className="mx-auto grid w-full max-w-(--page-max) gap-4 p-4 sm:p-6 lg:p-8"
        data-testid="world-section-shell"
      >
        <div className="flex justify-end">
          <Button
            asChild
            variant="primary"
            size="lg"
            icon="spark"
            data-testid="play-button"
          >
            <Link to={`/world/${worldId}/play`}>
              {scene ? `Resume in ${scene}` : "Play"}
            </Link>
          </Button>
        </div>
        {/* Spec 051 US5: here rather than on one page, so every hub screen a
            member lands on says that play is paused, and since when. */}
        <PlayPausedBanner worldId={worldId} />
        {/* Stacked on a narrow screen, side by side from `lg` — the same
            arrangement `AdminSectionShell` makes, for the same reason. A
            14rem rail beside the content leaves about 95px for the content on
            a 375px phone, which is not a narrow column: it is a page that
            scrolls sideways, and every card on these screens was being blamed
            for it. The rail is a strip above the content there instead. */}
        <div className="flex flex-col items-stretch gap-4 lg:flex-row lg:items-start lg:gap-6">
          <WorldSidebarNav worldId={worldId} isGm={isGm} />
          <div className="min-w-0 flex-1">{children}</div>
        </div>
      </div>
    </WorldAppearance>
  );
}
