import type { SheetFieldChange } from "@/api/sheetImport";
import { CERTAINTY_TEXT, fieldLabel, shownValue } from "./rows";

/**
 * One value the sheet will write: what the actor holds now, what the sheet
 * says, how sure the reader is, and the text it read it from.
 */
export function FieldRow({ field }: { field: SheetFieldChange }) {
  const changes = JSON.stringify(field.old) !== JSON.stringify(field.new);
  return (
    <li
      className="grid gap-0.5 border-b border-border py-2 text-sm last:border-b-0"
      data-testid={`sheet-import-field-${field.path}`}
      data-certainty={field.certainty.toLowerCase()}
    >
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <span className="font-medium capitalize">{fieldLabel(field)}</span>
        <span
          className={
            field.certainty === "READ"
              ? "text-xs text-muted-foreground"
              : "text-xs font-semibold text-amber-600 dark:text-amber-400"
          }
        >
          {CERTAINTY_TEXT[field.certainty]}
          {field.playState ? " · in play, kept" : ""}
        </span>
      </div>
      <div className="flex flex-wrap items-baseline gap-2">
        {changes ? (
          <>
            <span className="text-muted-foreground line-through">
              {shownValue(field.old)}
            </span>
            <span aria-hidden="true">→</span>
          </>
        ) : null}
        <span data-testid="sheet-import-field-new">
          {shownValue(field.new)}
        </span>
      </div>
      {field.reason ? (
        <p className="text-xs text-muted-foreground">{field.reason}</p>
      ) : null}
      {field.source ? (
        <p className="text-xs text-muted-foreground">
          Page {field.source.page}: “{field.source.text}”
        </p>
      ) : null}
    </li>
  );
}
