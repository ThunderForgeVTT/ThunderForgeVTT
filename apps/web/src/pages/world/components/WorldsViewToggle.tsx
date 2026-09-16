import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { cn } from "@/lib/utils";
import type { WorldsView } from "@/pages/world/worldsView";

interface WorldsViewToggleProps {
  view: WorldsView;
  onChange: (view: WorldsView) => void;
}

const OPTIONS: { value: WorldsView; label: string; icon: "worlds" | "map" }[] =
  [
    { value: "tiles", label: "Tiles", icon: "worlds" },
    { value: "table", label: "Table", icon: "map" },
  ];

/**
 * Flip the archive between tiles and a table.
 *
 * # Why two buttons and `aria-pressed`, not a `Select` or a tab strip
 *
 * A `Select` would hide the alternative behind a click, and the whole point
 * of this control is that a person who did not expect a table can see, at a
 * glance, that tiles are still there. Tabs are the other tempting shape and
 * are wrong for a different reason: tabs claim their panels are different
 * *content*, and a screen reader user who hears "tab 2 of 2" expects to find
 * something new there. This is one set of worlds drawn two ways, which is
 * exactly what a pair of toggle buttons says — each one announcing whether
 * it is currently pressed.
 *
 * Both buttons are always operable, including the one already pressed, so
 * the control never becomes a dead end: clicking "Tiles" while on tiles
 * still records the preference, which is the difference between a person who
 * chose tiles and a person who has never said.
 */
export function WorldsViewToggle({ view, onChange }: WorldsViewToggleProps) {
  return (
    <div
      role="group"
      aria-label="World archive view"
      className="inline-flex rounded-lg border border-border bg-secondary/40 p-1"
      data-testid="worlds-view-toggle"
    >
      {OPTIONS.map((option) => {
        const active = option.value === view;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={active}
            onClick={() => onChange(option.value)}
            data-testid={`worlds-view-${option.value}`}
            className={cn(
              "inline-flex items-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
              "focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none",
              active
                ? "bg-card text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <FantasyIcon name={option.icon} size={14} />
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
