/**
 * What a scene's grid is called, in one place.
 *
 * # Why "Gridless" and not "None"
 *
 * The stored values are `"square" | "hex" | "gridless"` (spec 022 FR-014),
 * and the picker used to offer them as *None*, Squares, Hexagons. "None" is
 * the wrong word twice over: it reads as *nothing chosen* — the empty state
 * of a picker rather than a choice inside it — and it hides the fact that
 * gridless is a real mode with its own rules (`SnapRule` refuses to snap on
 * it, distances are measured straight). The engine has called it
 * `GridKind::Gridless` all along; only the web said "None".
 *
 * A module rather than three inline strings, because the label was wrong in
 * one place and *missing* in another: the play dock printed the raw
 * `"gridless"` from the record. Two spellings of one fact is how they drift.
 */

export const GRID_TYPE_VALUES = ["gridless", "square", "hex"] as const;

export type GridTypeValue = (typeof GRID_TYPE_VALUES)[number];

const LABELS: Record<GridTypeValue, string> = {
  gridless: "Gridless",
  square: "Squares",
  hex: "Hexagons",
};

/** The picker's options, in the order a picker should offer them. */
export const GRID_TYPE_OPTIONS: { value: GridTypeValue; label: string }[] =
  GRID_TYPE_VALUES.map((value) => ({ value, label: LABELS[value] }));

/**
 * The reader's word for a stored grid type.
 *
 * An unrecognised value comes back untouched rather than as "Gridless": a
 * scene stored with a grid this build does not know about is not a scene
 * without a grid, and saying so would be a guess dressed as a fact.
 */
export function gridTypeLabel(value: string): string {
  return (LABELS as Record<string, string | undefined>)[value] ?? value;
}
