import { GenieSessionPanel } from "@/components/world/GenieSessionPanel/GenieSessionPanel";

export interface ClocksPanelProps {
  worldId: string;
  isGm: boolean;
  /** The world's game system id — clocks are Genie-specific today. */
  gameSystemId: string | null;
}

/**
 * Clocks & Timers.
 *
 * The only clock mechanic in the app today is Genie's (Session Wish Pool,
 * Doom Clock, Puzzle Clocks), which is a system-specific feature — so this
 * panel shows it for Genie worlds and says plainly that there is nothing
 * else yet elsewhere, rather than rendering an empty frame that looks
 * broken. When a system-agnostic timer lands it belongs here alongside it,
 * not inside the Genie pack.
 */
export function ClocksPanel({ worldId, isGm, gameSystemId }: ClocksPanelProps) {
  if (gameSystemId !== "genie") {
    return (
      <p className="text-sm text-muted-foreground" data-testid="clocks-panel">
        This world&apos;s system has no clocks or timers yet. Genie worlds get the Doom
        Clock and Puzzle Clocks here.
      </p>
    );
  }

  return (
    <div data-testid="clocks-panel">
      <GenieSessionPanel worldId={worldId} isGm={isGm} />
    </div>
  );
}
