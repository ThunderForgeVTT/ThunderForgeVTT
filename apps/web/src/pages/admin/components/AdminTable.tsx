import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

/**
 * A real table, in a container that scrolls instead of the page.
 *
 * # Why this exists
 *
 * The admin surfaces were drawn as stacked cards: one card per OAuth
 * provider, per GitHub application, per manifest key. Every card repeated the
 * same labels, so comparing two providers meant scrolling 700px and
 * remembering. The owner's words were "can you tabulate this, it's kind of
 * hard to read" — and the reason it is hard to read is that the data *is*
 * rows and columns and was being drawn as prose.
 *
 * # Semantics, not a grid of divs
 *
 * `<table>` with a real `<thead>`, because a screen reader announces "column
 * 2, Status" only for a table. A CSS grid with `role="row"` sprinkled on is
 * the version of this that passes a glance and fails a screen reader, and the
 * axe fixture over these pages is exactly the check that would not catch it —
 * a div soup with no roles is silent rather than wrong.
 *
 * # The scroll container is the accessible part
 *
 * At 375px these tables are wider than the viewport. The wrapper — not the
 * page — scrolls, and it is focusable with an accessible name, because a
 * region that scrolls must be reachable from the keyboard (WCAG 2.1.1). That
 * is the whole reason `label` is required rather than optional.
 */
export interface AdminTableProps {
  /** Names the scrollable region and the table itself. Required. */
  label: string;
  /** One per column, in order. */
  columns: readonly string[];
  /** Column widths, `<col>`-style, so a value column can take the slack. */
  columnWidths?: readonly (string | undefined)[];
  children: ReactNode;
  className?: string;
  "data-testid"?: string;
}

export function AdminTable({
  label,
  columns,
  columnWidths,
  children,
  className,
  "data-testid": testId,
}: AdminTableProps) {
  return (
    <div
      // `tabIndex` on a scroll container is the WCAG 2.1.1 fix, not a
      // decoration: without it the only way to see the right-hand columns at
      // 375px is a pointer drag.
      tabIndex={0}
      role="region"
      aria-label={label}
      className={cn(
        "overflow-x-auto rounded-lg border border-border",
        "focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none",
        className,
      )}
      data-testid={testId}
    >
      <table className="w-full min-w-[34rem] border-collapse text-left text-sm">
        {columnWidths ? (
          <colgroup>
            {columns.map((column, index) => (
              <col key={column} style={{ width: columnWidths[index] }} />
            ))}
          </colgroup>
        ) : null}
        <caption className="sr-only">{label}</caption>
        <thead>
          <tr className="border-b border-border bg-secondary/60">
            {columns.map((column) => (
              <th
                key={column}
                scope="col"
                className="px-3 py-2.5 text-xs font-semibold tracking-wider text-muted-foreground uppercase"
              >
                {column}
              </th>
            ))}
          </tr>
        </thead>
        {children}
      </table>
    </div>
  );
}

/**
 * The cell a row's disclosure lives in, and the row it opens.
 *
 * Kept here so all three tables open the same way: a button in the last cell,
 * `aria-expanded`, and the detail as a full-width row directly beneath — which
 * is the one place a detail can go without breaking the table's column count.
 */
export function AdminDetailRow({
  columnCount,
  children,
  ...rest
}: {
  columnCount: number;
  children: ReactNode;
  "data-testid"?: string;
}) {
  return (
    <tr className="border-b border-border last:border-b-0" {...rest}>
      <td colSpan={columnCount} className="bg-secondary/20 px-3 py-4">
        {children}
      </td>
    </tr>
  );
}
