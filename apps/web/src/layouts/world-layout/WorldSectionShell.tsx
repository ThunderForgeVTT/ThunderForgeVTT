import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
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
 * Width is deliberately fluid rather than the app's usual `Container`
 * (max-w-[1160px]) — these are information-dense hub pages (catalogs, a
 * Markdown editor) sitting behind a fixed-width sidebar, so a 1160px cap
 * left a lot of dead space on wide monitors. `max-w-[1800px]` still caps
 * line length on ultrawide screens; `w-full` lets it shrink freely below
 * that on anything smaller. Play's own full-screen canvas is unrelated
 * chrome and isn't wrapped by this component.
 */
export function WorldSectionShell({ worldId, isGm, children }: WorldSectionShellProps) {
  return (
    <div className="mx-auto grid w-full max-w-[1800px] gap-4 p-4 sm:p-6 lg:p-8">
      <div className="flex justify-end">
        <Button asChild variant="primary" size="lg" icon="spark" data-testid="play-button">
          <Link to={`/world/${worldId}/play`}>Play</Link>
        </Button>
      </div>
      <div className="flex items-start gap-6">
        <WorldSidebarNav worldId={worldId} isGm={isGm} />
        <div className="min-w-0 flex-1">{children}</div>
      </div>
    </div>
  );
}
