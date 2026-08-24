import type { ReactNode } from "react";
import { WorldSidebarNav } from "@/layouts/world-layout/WorldSidebarNav";

export interface WorldSectionShellProps {
  worldId: string;
  isGm: boolean;
  children: ReactNode;
}

/**
 * Shared chrome for the world's "hub" pages (Session Setup, Compendium,
 * System settings) — a persistent category sidebar alongside whatever the
 * page itself renders. Kept as a thin wrapper (not a nested route layout)
 * so each route page's existing data-fetching is untouched.
 */
export function WorldSectionShell({ worldId, isGm, children }: WorldSectionShellProps) {
  return (
    <div className="mx-auto flex w-full max-w-6xl items-start gap-6 p-4 sm:p-6">
      <WorldSidebarNav worldId={worldId} isGm={isGm} />
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  );
}
