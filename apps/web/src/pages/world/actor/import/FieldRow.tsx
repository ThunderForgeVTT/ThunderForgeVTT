import { useState } from "react";
import type { SheetFieldChange } from "@/api/sheetImport";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import {
  CERTAINTY_TEXT,
  CHANGE_TEXT,
  changeKind,
  correctable,
  fieldLabel,
  parseCorrection,
  shownValue,
} from "./rows";

/**
 * One value the sheet will write: what the actor holds now, what the sheet
 * says, whether accepting replaces something, how sure the reader is, and
 * the text it read it from. A value the reader was not sure of can be
 * corrected here (FR-022).
 */
export function FieldRow({
  field,
  onCorrect,
  disabled = false,
}: {
  field: SheetFieldChange;
  onCorrect?: (path: string, value: unknown) => void;
  disabled?: boolean;
}) {
  const change = changeKind(field);
  return (
    <li
      className="grid gap-0.5 border-b border-border py-2 text-sm last:border-b-0"
      data-testid={`sheet-import-field-${field.path}`}
      data-certainty={field.certainty.toLowerCase()}
      data-change={change}
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
        {change !== "same" ? (
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
        {change !== "same" ? (
          <span
            className={
              change === "overwrites"
                ? "rounded bg-amber-500/15 px-1.5 text-xs font-medium text-amber-700 dark:text-amber-300"
                : "rounded bg-muted px-1.5 text-xs text-muted-foreground"
            }
            data-testid="sheet-import-field-change"
          >
            {CHANGE_TEXT[change]}
          </span>
        ) : null}
      </div>
      {field.reason ? (
        <p className="text-xs text-muted-foreground">{field.reason}</p>
      ) : null}
      {field.source ? (
        <p className="text-xs text-muted-foreground">
          Page {field.source.page}: “{field.source.text}”
        </p>
      ) : null}
      {onCorrect && correctable(field) ? (
        <Correction field={field} onCorrect={onCorrect} disabled={disabled} />
      ) : null}
    </li>
  );
}

function Correction({
  field,
  onCorrect,
  disabled,
}: {
  field: SheetFieldChange;
  onCorrect: (path: string, value: unknown) => void;
  disabled: boolean;
}) {
  const [text, setText] = useState("");
  const value = parseCorrection(field, text);
  const label = `Correct ${fieldLabel(field)}`;
  return (
    <form
      className="mt-1 flex flex-wrap items-center gap-2"
      onSubmit={(event) => {
        event.preventDefault();
        if (value !== undefined) onCorrect(field.path, value);
      }}
    >
      <Input
        aria-label={label}
        className="h-8 min-w-0 flex-1"
        value={text}
        placeholder={shownValue(field.new)}
        onChange={(event) => setText(event.target.value)}
        disabled={disabled}
        data-testid={`sheet-import-correct-${field.path}`}
      />
      <Button
        type="submit"
        size="sm"
        variant="secondary"
        disabled={disabled || value === undefined}
        data-testid={`sheet-import-correct-${field.path}-use`}
      >
        Use this
      </Button>
    </form>
  );
}
