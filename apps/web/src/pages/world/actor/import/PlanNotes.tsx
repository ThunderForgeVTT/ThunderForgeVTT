import type {
  SheetCrossCheck,
  SheetKeptInPlay,
  SheetUnmapped,
} from "@/api/sheetImport";
import { Card } from "@/components/ui/card/Card";
import { goesToText, shownValue } from "./rows";

/** `derived.skill.perception` reads as "perception". */
function checkLabel(path: string): string {
  return (path.split(".").pop() ?? path).replace(/_/g, " ");
}

/**
 * Numbers the sheet prints that the rules work out differently. Neither is
 * written: the system derives its own, and the field it comes from is
 * marked for checking (FR-020).
 */
export function CrossChecks({ checks }: { checks: SheetCrossCheck[] }) {
  if (checks.length === 0) return null;
  return (
    <Card className="grid gap-2 p-4">
      <h2 className="font-semibold">The sheet and the rules disagree</h2>
      <ul className="grid gap-1 text-sm" data-testid="sheet-import-crosschecks">
        {checks.map((check) => (
          <li
            key={check.path}
            className="flex flex-wrap items-baseline gap-2"
            data-testid={`sheet-import-crosscheck-${check.path}`}
          >
            <span className="font-medium capitalize">
              {checkLabel(check.path)}
            </span>
            <span>
              the sheet prints{" "}
              <span data-testid="sheet-import-crosscheck-sheet">
                {shownValue(check.sheet)}
              </span>
              , the rules give{" "}
              <span data-testid="sheet-import-crosscheck-derived">
                {shownValue(check.derived)}
              </span>
            </span>
          </li>
        ))}
      </ul>
    </Card>
  );
}

/**
 * On a re-import, values in play (hit points, slots spent) are kept unless
 * the person ticks them here.
 */
export function KeptInPlayList({
  kept,
  overwrite,
  onToggle,
  disabled,
}: {
  kept: SheetKeptInPlay[];
  overwrite: string[];
  onToggle: (target: string, on: boolean) => void;
  disabled: boolean;
}) {
  if (kept.length === 0) return null;
  return (
    <Card className="grid gap-2 p-4">
      <h2 className="font-semibold">In play</h2>
      <p className="text-sm text-muted-foreground">
        These stay as they are in play unless you tick them.
      </p>
      <ul className="grid gap-1 text-sm" data-testid="sheet-import-kept">
        {kept.map((row) => (
          <li key={row.target}>
            <label className="flex flex-wrap items-center gap-2">
              <input
                type="checkbox"
                checked={overwrite.includes(row.target)}
                onChange={(event) => onToggle(row.target, event.target.checked)}
                disabled={disabled}
                data-testid={`sheet-import-kept-${row.target}`}
              />
              <span className="capitalize">
                {goesToText(row.target)}: {shownValue(row.current)} in play,{" "}
                {shownValue(row.sheet)} on the sheet. Use the sheet's.
              </span>
            </label>
          </li>
        ))}
      </ul>
    </Card>
  );
}

/** What the system does not track, and where each value goes (FR-013). */
export function UnmappedList({ unmapped }: { unmapped: SheetUnmapped[] }) {
  if (unmapped.length === 0) return null;
  return (
    <Card className="grid gap-2 p-4">
      <h2 className="font-semibold">Not tracked by this system</h2>
      <p
        className="text-sm text-muted-foreground"
        data-testid="sheet-import-unmapped"
      >
        {unmapped.length} value{unmapped.length === 1 ? "" : "s"} the system
        does not track will go into the notes.
      </p>
      <ul
        className="grid gap-1 text-sm"
        data-testid="sheet-import-unmapped-list"
      >
        {unmapped.map((row) => (
          <li
            key={row.path}
            className="flex flex-wrap items-baseline justify-between gap-2"
            data-testid={`sheet-import-unmapped-${row.path}`}
          >
            <span>{shownValue(row.value)}</span>
            <span className="text-xs text-muted-foreground">
              into {goesToText(row.goesTo)}
            </span>
          </li>
        ))}
      </ul>
    </Card>
  );
}
