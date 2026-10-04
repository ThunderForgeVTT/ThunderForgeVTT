import { createElement } from "react";
import { resolvePanel } from "@/panels/systemPanels";

export interface ClocksPanelProps {
  worldId: string;
  isGm: boolean;
  /** Who is looking, passed through to whichever pack fills the slot. */
  currentUserId?: string;
  /** The world's game system id — used only to look the slot up. */
  gameSystemId: string | null;
}

/**
 * Clocks & Timers.
 *
 * # What changed, and why it is not a relocation
 *
 * This panel used to compare `gameSystemId` against one system's id and
 * print an empty state when it did not match, rendering that system's
 * session panel when it did. It was the fourth of
 * the four violations `check-system-registry.mjs` listed against `032/T108`,
 * and the only one wearing the opposite sign: it did not mount a panel *for*
 * a named system so much as refuse to mount one for everybody else. Same
 * comparison, same rule broken.
 *
 * The question it asks now is "did any pack contribute a clocks panel?" and
 * the empty state is what it prints when none did. That deletes the
 * comparison rather than moving it somewhere less visible — there is no
 * system id left to compare, only a lookup that either finds something or
 * does not.
 *
 * The empty state had to lose its second sentence with it. It used to name
 * the one system that fills this slot and the two clocks it brings, which was
 * true and which this component has no business knowing; a page that
 * advertises a particular system is the same violation in prose. What is left
 * says
 * the honest thing — this system has no clocks — rather than advertising
 * another one's.
 */
export function ClocksPanel({
  worldId,
  isGm,
  currentUserId,
  gameSystemId,
}: ClocksPanelProps) {
  const Panel = resolvePanel(gameSystemId, "clocks");

  if (!Panel) {
    return (
      <p className="text-sm text-muted-foreground" data-testid="clocks-panel">
        This world&apos;s system has no clocks or timers yet.
      </p>
    );
  }

  return (
    <div data-testid="clocks-panel">
      {createElement(Panel, { worldId, isGm, currentUserId })}
    </div>
  );
}
