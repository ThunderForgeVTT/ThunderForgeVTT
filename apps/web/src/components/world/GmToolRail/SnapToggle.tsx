import { useEffect, useState } from "react";
import { onGridSnapChanged, setGridSnap } from "@/engine/bevy";
import { cn } from "@/lib/utils";

/**
 * The snapping switch (spec 077 FR-001): one button in the rail, shown while
 * an authoring tool is open, reading the engine's `GridSnapEnabled` and
 * flipping it on click. `S` does the same from the keyboard, and the tooltip
 * says so.
 *
 * The state is the engine's, not this component's: it subscribes to
 * `grid_snap_changed` and asks for a change rather than remembering one, so
 * the key and the button cannot disagree (SC-003). Session state like
 * `setIsGameMaster` — never a store command, never synced.
 */
export function SnapToggle() {
  const [enabled, setEnabled] = useState(true);

  useEffect(() => onGridSnapChanged((event) => setEnabled(event.enabled)), []);

  const label = enabled ? "Snapping on" : "Snapping off";
  return (
    <button
      type="button"
      title={`${label} (S)`}
      aria-label={label}
      aria-pressed={enabled}
      data-testid="gm-snap-toggle"
      data-enabled={enabled ? "true" : "false"}
      onClick={() => {
        void setGridSnap(!enabled);
      }}
      className={cn(
        "mt-auto flex h-9 w-9 items-center justify-center rounded-lg transition-colors",
        enabled
          ? "bg-primary/20 text-primary hover:bg-primary/30"
          : "text-muted-foreground hover:bg-muted hover:text-foreground",
      )}
    >
      {/* A lattice with a point on its corner: what snapping lands on. */}
      <svg
        width="18"
        height="18"
        viewBox="0 0 18 18"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        aria-hidden="true"
      >
        <path d="M2 6h14M2 12h14M6 2v14M12 2v14" opacity={enabled ? 1 : 0.5} />
        <circle cx="12" cy="6" r="2.5" fill="currentColor" stroke="none" />
      </svg>
    </button>
  );
}
