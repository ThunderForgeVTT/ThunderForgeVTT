/**
 * Turning what a system declares into what the renderer resolves against.
 *
 * Two lookups, and one subtraction: a set by kind (for generic constructs,
 * which name nothing), a value by identifier (for specific ones, which do),
 * and `other` — everything the named sets left unclaimed.
 */

import {
  DECLARATION_SETS,
  NAMED_DECLARATION_SETS,
  type DeclarationSet,
  type ResolvedDeclarations,
  type SheetDeclarations,
  type SheetValue,
} from "./types";

/** A declaration set with every kind present and every kind empty. */
export function emptyDeclarations(): ResolvedDeclarations {
  return {
    attributes: [],
    resources: [],
    skills: [],
    movement: [],
    derived: [],
    other: [],
  };
}

/**
 * Fill in whichever sets a caller left out, and work out `other`.
 *
 * A system that declares no skills and a caller that simply did not pass any
 * are the same thing to a layout: there is nothing to draw. Both arrive here
 * as an empty set rather than as `undefined`, so the renderer has one case to
 * handle instead of two.
 *
 * `other` is computed here rather than supplied, because it is a complement
 * and a caller cannot be trusted to have kept it in step: the whole point of
 * FR-034 is that a value nobody classified still reaches the sheet. Anything
 * in `all` whose identifier no named set claims is one of those, in the
 * system's own declaration order.
 */
export function declarationsFrom(
  partial: Partial<SheetDeclarations> | undefined,
): ResolvedDeclarations {
  const full = emptyDeclarations();
  if (!partial) return full;

  const claimed = new Set<string>();
  for (const set of NAMED_DECLARATION_SETS) {
    const values = partial[set] ?? [];
    full[set] = values;
    for (const value of values) claimed.add(value.id);
  }

  const other: SheetValue[] = [];
  const seen = new Set<string>();
  for (const value of partial.all ?? []) {
    if (claimed.has(value.id) || seen.has(value.id)) continue;
    seen.add(value.id);
    other.push(value);
  }
  full.other = other;

  return full;
}

/**
 * Every declared value by identifier.
 *
 * Built across all sets — `other` included — because a specific construct
 * names an identifier without saying which set it came from: `deathSaves` is
 * a track whether the system files it under resources, derived, or nowhere
 * at all.
 *
 * First declaration wins on a collision: a value listed in two sets is one
 * value, and the earlier set is the one the system reached for first.
 */
export function indexById(
  declarations: ResolvedDeclarations,
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
  declarations: ResolvedDeclarations,
  set: DeclarationSet,
): readonly SheetValue[] {
  return declarations[set] ?? [];
}
