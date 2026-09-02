import { useEffect, useState } from "react";
import { setSelectionFilter } from "@/engine/bevy";

/**
 * Which kinds the Select tool acts on.
 *
 * # Why the Select tool has a panel now, when it deliberately had none
 *
 * Select was given no flyout on purpose: "a panel open by default would cover
 * 256px of map on every load to say you may now click things." That reasoning
 * still holds, and this does not contradict it — the menu is collapsible and
 * remembers being collapsed, so a Game Master who does not want it never sees
 * it again after the first time.
 *
 * What it buys is the complaint that produced it: moving tokens around a
 * finished map keeps catching walls and lights instead.
 *
 * # Why the state lives here and not in the world
 *
 * This is a working preference of the person at the keyboard, not a property
 * of the world. Two Game Masters on one world must not fight over it, and
 * nothing else needs to know. So it is per-user, per-device, and the server
 * never hears about it (spec 031 FR-009, research R10).
 *
 * The engine remains the authority for what a click *does* — selection is
 * engine state. This component only decides what to ask for.
 */

export interface SelectionKinds {
  tokens: boolean;
  walls: boolean;
  lights: boolean;
  shapes: boolean;
}

const STORAGE_KEY = "thunderforge.selectionFilter";
const COLLAPSED_KEY = "thunderforge.selectionFilter.collapsed";

/** Everything on. A restrictive default reads as a broken tool. */
const ALL_ON: SelectionKinds = {
  tokens: true,
  walls: true,
  lights: true,
  shapes: true,
};

const KINDS: { key: keyof SelectionKinds; label: string }[] = [
  { key: "tokens", label: "Tokens" },
  { key: "walls", label: "Walls" },
  { key: "lights", label: "Lights" },
  { key: "shapes", label: "Shapes" },
];

function readStored<T>(key: string, fallback: T): T {
  // Storage can throw outright in a hardened context, not merely return null.
  // A preference is never worth taking the page down for.
  try {
    const raw = window.localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}

function writeStored(key: string, value: unknown): void {
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // A preference that cannot be remembered is a preference that resets, not
    // an error the user can do anything about.
  }
}

export function SelectionFilterMenu() {
  const [kinds, setKinds] = useState<SelectionKinds>(() =>
    readStored(STORAGE_KEY, ALL_ON),
  );
  const [collapsed, setCollapsed] = useState<boolean>(() =>
    readStored(COLLAPSED_KEY, false),
  );

  // Tell the engine, and remember. Runs on mount too, so a stored filter is in
  // force from the first click rather than from the first change.
  useEffect(() => {
    writeStored(STORAGE_KEY, kinds);
    void setSelectionFilter(kinds.tokens, kinds.walls, kinds.lights, kinds.shapes);
  }, [kinds]);

  useEffect(() => {
    writeStored(COLLAPSED_KEY, collapsed);
  }, [collapsed]);

  const selectsNothing =
    !kinds.tokens && !kinds.walls && !kinds.lights && !kinds.shapes;

  if (collapsed) {
    return (
      <button
        type="button"
        data-testid="selection-filter-expand"
        data-selects-nothing={selectsNothing ? "true" : "false"}
        className={
          selectsNothing
            ? "text-xs text-destructive underline"
            : "text-xs text-muted-foreground underline"
        }
        onClick={() => setCollapsed(false)}
      >
        {/*
          Collapsed *and* selecting nothing is the one combination that is
          genuinely invisible: the Select tool stops responding and there is
          nothing on screen saying why. The expanded panel explains itself, and
          the rail is the only place left to say it when the panel is shut.

          So the collapsed control carries the state rather than a fixed label
          — spec 031's edge case is about a tool that appears broken, and a
          person who collapsed this menu an hour ago is exactly the person who
          will not remember switching everything off.
        */}
        {selectsNothing ? "Selection filter — nothing selectable" : "Selection filter"}
      </button>
    );
  }

  return (
    <div className="grid gap-2" data-testid="selection-filter">
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs font-medium">Select acts on</span>
        <button
          type="button"
          data-testid="selection-filter-collapse"
          className="text-xs text-muted-foreground underline"
          onClick={() => setCollapsed(true)}
        >
          Collapse
        </button>
      </div>

      {KINDS.map(({ key, label }) => (
        <label key={key} className="flex items-center gap-2 text-xs">
          <input
            type="checkbox"
            data-testid={`selection-filter-${key}`}
            checked={kinds[key]}
            onChange={(event) =>
              setKinds((current) => ({ ...current, [key]: event.target.checked }))
            }
          />
          {label}
        </label>
      ))}

      {selectsNothing ? (
        /*
          Excluding everything is a legitimate state and an invisible one: the
          tool simply stops responding, which is indistinguishable from broken.
          Spec 031's edge case asks for it to be obvious, so it is said plainly
          rather than left to be inferred from clicks doing nothing.
        */
        <p
          className="text-xs text-muted-foreground"
          data-testid="selection-filter-empty"
        >
          Nothing is selectable while every kind is unchecked.
        </p>
      ) : null}
    </div>
  );
}
