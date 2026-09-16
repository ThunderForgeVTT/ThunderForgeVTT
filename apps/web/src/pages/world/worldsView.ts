/**
 * Tiles or a table, for the world archive.
 *
 * # Why a stored preference and a threshold, not one or the other
 *
 * The owner's words: "if somebody has more than six worlds, we should convert
 * to a table, but allow them to flip between table or tiled view". Those are
 * two rules, and they answer different questions. The threshold answers
 * "what should a person who has never expressed a preference see?" — six
 * tiles is a wall you can still read, seven is a list you are scrolling. The
 * stored preference answers "what should a person who *has* expressed one
 * see?" — and it wins at every count, in both directions, because a person
 * who chose tiles at nine worlds meant it.
 *
 * So the stored value is deliberately three-state: `"tiles"`, `"table"`, or
 * *absent*. Absent is not "tiles"; it is "nobody has said", which is the only
 * state in which the count gets to decide. Collapsing that to a boolean would
 * make the threshold fire exactly once — on a person's very first visit — and
 * never again for anyone who had ever touched the control.
 */

export type WorldsView = "tiles" | "table";

const STORAGE_KEY = "tf:worlds-view";

/** Above this many worlds, tiles stop being something you can take in. */
export const TILE_VIEW_LIMIT = 6;

function isWorldsView(value: string | null): value is WorldsView {
  return value === "tiles" || value === "table";
}

/**
 * The view this person last chose, or `null` if they never have.
 *
 * `null` is also what a blocked or cleared store gives back, and that is the
 * correct answer rather than a fallback: a browser that cannot remember has,
 * as far as this page can tell, nobody who has chosen.
 */
export function readStoredWorldsView(): WorldsView | null {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    return isWorldsView(stored) ? stored : null;
  } catch {
    return null;
  }
}

/** Best-effort. Private browsing just means the choice lasts one visit. */
export function storeWorldsView(view: WorldsView): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, view);
  } catch {
    // Nothing to do and nothing to tell the person: the view they clicked is
    // already the view they are looking at.
  }
}

/**
 * What to draw: the choice if there is one, the count's verdict otherwise.
 *
 * Kept as a function rather than inlined at the call site so the e2e and unit
 * tests can state the rule once — seven is the first count that draws a
 * table, and a preference beats any count.
 */
export function resolveWorldsView(
  chosen: WorldsView | null,
  worldCount: number,
): WorldsView {
  if (chosen) {
    return chosen;
  }
  return worldCount > TILE_VIEW_LIMIT ? "table" : "tiles";
}
