import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { getResumeSceneName } from "@/api/worldStatistics";
import { Button } from "@/components/ui/button/Button";
import { WorldAppearance } from "@/appearance/WorldAppearance";
import { PlayPausedBanner } from "@/components/world/PlayPausedBanner";
import { useWorldChrome } from "@/hooks/world-chrome-context";
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
 * Kept as a thin wrapper (not a nested route layout); what the pages read
 * again on every arrival (the world, its members, the play state) comes
 * from the shared world store (spec 101), shown at once and refreshed
 * behind the page. Play lives here (rather than in the
 * sidebar's category list or duplicated per-page) so every screen gets
 * exactly one, in the same place.
 *
 * Play names the scene it resumes in (spec 100 US3.4) on every page here,
 * not only the overview. The name is shared app state (`useWorldChrome`), so
 * moving between pages shows the last one at once while it is refreshed; it
 * says "Play" before the first answer, or when no scene is active.
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
  const scene = useWorldChrome(worldId, "resumeScene", getResumeSceneName);

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
