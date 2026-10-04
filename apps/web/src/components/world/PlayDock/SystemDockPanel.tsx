import { createElement } from "react";
import { resolvePanel } from "@/panels/systemPanels";

export interface SystemDockPanelProps {
  worldId: string;
  isGm: boolean;
  /** Who is looking, passed through to whichever pack fills the slot. */
  currentUserId?: string;
  /** The world's game system id — used only to look the slot up. */
  gameSystemId: string | null;
}

/**
 * The play dock's section a game system may fill (spec 067 Story 2).
 *
 * This was `ClocksPanel`, and the slot was `clocks`, because the first pack
 * to fill it brought clocks. The second brought a difficulty target, and a
 * tab called "Clocks & Timers" opened onto something that was neither. The
 * slot is named for where it mounts now, and the pack says what it holds.
 *
 * It still names no system: a lookup that finds a panel or does not.
 */
export function SystemDockPanel({
  worldId,
  isGm,
  currentUserId,
  gameSystemId,
}: SystemDockPanelProps) {
  const Panel = resolvePanel(gameSystemId, "dock");

  if (!Panel) {
    return null;
  }

  return (
    <div data-testid="system-dock-panel">
      {createElement(Panel, { worldId, isGm, currentUserId })}
    </div>
  );
}
