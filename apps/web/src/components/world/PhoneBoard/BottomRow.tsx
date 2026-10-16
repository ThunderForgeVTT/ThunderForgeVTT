import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { cn } from "@/lib/utils";
import type { RowItem } from "./phoneRows";

export interface BottomRowProps {
  items: RowItem[];
  /** The item whose sheet is open, if any. */
  openId: string | null;
  onPick: (item: RowItem) => void;
  /** A count to badge an item with, by id (FR-011: requests on Tools). */
  badges?: Partial<Record<string, number>>;
}

/**
 * Spec 102 R6: five tiles where a thumb rests. Bottom centre in portrait;
 * a column at the right edge in landscape, where height is the short side.
 * Each tile floats on its own, so the map shows between them.
 */
export function BottomRow({ items, openId, onPick, badges }: BottomRowProps) {
  return (
    <nav
      aria-label="Board"
      data-testid="phone-row"
      className={cn(
        "pointer-events-none fixed z-[1065] flex gap-2",
        "bottom-[max(12px,env(safe-area-inset-bottom))] left-1/2 -translate-x-1/2",
        "landscape:top-1/2 landscape:right-[max(12px,env(safe-area-inset-right))] landscape:bottom-auto landscape:left-auto landscape:translate-x-0 landscape:-translate-y-1/2 landscape:flex-col",
      )}
    >
      {items.map((item) => {
        const open = item.id === openId;
        const badge = badges?.[item.id] ?? 0;
        return (
          <button
            key={item.id}
            type="button"
            data-testid={`phone-row-${item.id}`}
            aria-expanded={open}
            onClick={() => onPick(item)}
            className={cn(
              "pointer-events-auto relative flex h-[52px] w-14 flex-col items-center justify-center gap-0.5 rounded-2xl border shadow-lg backdrop-blur transition-colors duration-[180ms] motion-reduce:transition-none",
              open
                ? "border-primary bg-primary text-primary-foreground"
                : "border-border bg-background/85 text-foreground active:bg-muted",
            )}
          >
            <FantasyIcon name={item.icon} size={20} />
            <span className="text-[11px] leading-none font-medium">
              {item.label}
            </span>
            {badge > 0 ? (
              <span
                data-testid={`phone-row-${item.id}-badge`}
                className="absolute -top-1.5 -right-1.5 flex h-5 min-w-5 items-center justify-center rounded-full bg-destructive px-1 text-[11px] font-semibold text-white tabular-nums"
              >
                {badge}
                <span className="sr-only"> waiting</span>
              </span>
            ) : null}
          </button>
        );
      })}
    </nav>
  );
}
