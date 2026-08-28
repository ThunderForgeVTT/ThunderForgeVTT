import type { ReactNode } from "react";

interface WorldLayoutProps {
  worldId: string;
  canvas: ReactNode;
  /** The GM's left-hand tool rail. Omitted entirely for players. */
  toolRail?: ReactNode;
  /** The right-hand dock (Chat / Actors / Combat / Clocks / Settings). */
  dock?: ReactNode;
}

/**
 * Full-screen canvas shell for the Play view: the canvas fills the
 * viewport, with the GM tool rail docked left and the panel dock docked
 * right on top of it.
 *
 * This used to own the chrome itself — a bottom-left "Back to setup" /
 * "Tools" button pair and a single collapsible sidebar containing scenes,
 * the NPC roster, and a trackers/settings tab strip. All of that moved into
 * the two docks (`GmToolRail`, `WorldDock`), so this component is now just
 * the layering: it decides what sits above the canvas and in which corner,
 * and nothing about what those panels contain.
 *
 * The canvas is passed in rather than rendered here, and stays mounted
 * across view changes — invalidating the already-booted Bevy engine's
 * canvas handle is exactly what spec 009's research.md §1 found load-
 * bearing not to do.
 */
export function WorldLayout({
  worldId,
  canvas,
  toolRail,
  dock,
}: WorldLayoutProps) {
  return (
    <div
      className="relative h-screen w-screen overflow-hidden"
      data-world-id={worldId}
    >
      <div className="absolute inset-0">{canvas}</div>
      {toolRail}
      {dock}
    </div>
  );
}
