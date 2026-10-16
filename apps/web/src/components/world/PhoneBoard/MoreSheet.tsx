import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { cn } from "@/lib/utils";
import type { RowItem } from "./phoneRows";

export interface MoreSheetProps {
  groups: { board: RowItem[]; table: RowItem[] };
  /** The armed tool, marked as current. */
  armedId?: string | null;
  onPick: (item: RowItem) => void;
  badges?: Partial<Record<string, number>>;
}

/**
 * Spec 102: everything that does not earn a tile. The Game Master's Tools
 * sheet (Board, then Table) and a player's More sheet are the same list;
 * an empty group is left out rather than shown with no rows.
 */
export function MoreSheet({ groups, armedId, onPick, badges }: MoreSheetProps) {
  const sets = [
    { id: "board", label: "Board", items: groups.board },
    { id: "table", label: "Table", items: groups.table },
  ].filter((g) => g.items.length > 0);
  const titled = sets.length > 1;

  return (
    <div className="flex flex-col gap-4">
      {sets.map((group) => (
        <div
          key={group.id}
          role="group"
          aria-labelledby={titled ? `phone-group-${group.id}-label` : undefined}
          aria-label={titled ? undefined : group.label}
          data-testid={`phone-group-${group.id}`}
          className="flex flex-col gap-1"
        >
          {titled ? (
            <h3
              id={`phone-group-${group.id}-label`}
              className="px-1 text-xs font-medium tracking-wide text-muted-foreground uppercase"
            >
              {group.label}
            </h3>
          ) : null}
          {group.items.map((item) => {
            const armed = item.id === armedId;
            const badge = badges?.[item.id] ?? 0;
            return (
              <button
                key={item.id}
                type="button"
                data-testid={`phone-item-${item.id}`}
                aria-current={armed ? "true" : undefined}
                onClick={() => onPick(item)}
                className={cn(
                  "flex min-h-12 items-center gap-3 rounded-xl px-3 text-left text-sm transition-colors",
                  armed
                    ? "bg-primary/15 font-semibold text-foreground"
                    : "active:bg-muted",
                )}
              >
                <FantasyIcon name={item.icon} size={20} />
                <span className="flex-1">{item.label}</span>
                {badge > 0 ? (
                  <span className="rounded-full bg-destructive px-2 text-xs font-semibold text-white tabular-nums">
                    {badge}
                  </span>
                ) : null}
              </button>
            );
          })}
        </div>
      ))}
    </div>
  );
}
