/**
 * Turning what a system declares into what the renderer resolves against.
 *
 * Two lookups, and nothing else: a set by kind (for generic constructs, which
 * name nothing) and a value by identifier (for specific ones, which do).
 */

import {
  DECLARATION_SETS,
  type DeclarationSet,
  type SheetDeclarations,
  type SheetValue,
} from "./types";

/** A declaration set with every kind present and every kind empty. */
export function emptyDeclarations(): SheetDeclarations {
  return {
    attributes: [],
    resources: [],
    skills: [],
    movement: [],
    derived: [],
  };
}

/**
 * Fill in whichever sets a caller left out.
 *
 * A system that declares no skills and a caller that simply did not pass any
 * are the same thing to a layout: there is nothing to draw. Both arrive here
 * as an empty set rather than as `undefined`, so the renderer has one case to
 * handle instead of two.
 */
export function declarationsFrom(
  partial: Partial<SheetDeclarations> | undefined,
): SheetDeclarations {
  const full = emptyDeclarations();
  if (!partial) return full;
  for (const set of DECLARATION_SETS) {
    full[set] = partial[set] ?? [];
  }
  return full;
}

/**
 * Every declared value by identifier.
 *
 * Built across all sets, because a specific construct names an identifier
 * without saying which set it came from — `deathSaves` is a tracker whether
 * the system files it under resources or derived.
 *
 * First declaration wins on a collision: a value listed in two sets is one
 * value, and the earlier set is the one the system reached for first.
 */
export function indexById(
  declarations: SheetDeclarations,
): ReadonlyMap<string, SheetValue> {
  const index = new Map<string, SheetValue>();
  for (const set of DECLARATION_SETS) {
    for (const value of declarations[set]) {
      if (!index.has(value.id)) index.set(value.id, value);
    }
  }
  return index;
}

/** The declarations in a set, in the system's own order. */
export function valuesIn(
  declarations: SheetDeclarations,
  set: DeclarationSet,
): readonly SheetValue[] {
  return declarations[set] ?? [];
}
