import type { SheetContentChange } from "@/api/sheetImport";
import { contentText } from "./rows";

/**
 * One spell, feature or item the sheet links: the world's own, or a piece
 * the GM will be asked about before it can be used in play.
 */
export function ContentRow({ change }: { change: SheetContentChange }) {
  const staged = !change.removed && change.resolution !== "WORLD";
  return (
    <li
      className="flex flex-wrap items-baseline justify-between gap-2 py-1 text-sm"
      data-testid={`sheet-import-content-${change.kind}-${change.name}`}
      data-resolution={change.resolution.toLowerCase()}
      data-removed={change.removed ? "yes" : "no"}
    >
      <span className={change.removed ? "line-through" : undefined}>
        {change.name}
      </span>
      <span
        className={
          staged
            ? "text-xs font-semibold text-amber-600 dark:text-amber-400"
            : "text-xs text-muted-foreground"
        }
      >
        {contentText(change)}
      </span>
    </li>
  );
}
