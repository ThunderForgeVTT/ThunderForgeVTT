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

  reportDrift(partial, claimed);

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
 * Identifiers a named set claims that `all` does not contain (T019h).
 *
 * Six lists arrive and nothing checks they agree with each other. Five of them
 * are the named sets and the sixth is `all`, which is supposed to be
 * everything the system publishes — and `other` is computed as the part of
 * `all` that the named sets did not claim.
 *
 * So an id in a named set but missing from `all` proves `all` is *not* the
 * whole published set. Nothing visibly breaks in that case, which is what
 * makes it worth reporting: the value in the named set still renders, and it
 * is the values in **no** named set that `all` is then also likely to be
 * missing — and those are the ones that vanish, which is precisely FR-035's
 * failure, absence being indistinguishable from the character not having it.
 *
 * Exported so a test can assert the condition rather than watch for a console
 * line. This is a detector, not the fix: carrying set membership on each value
 * would make the six lists one, and that is a wire change beyond this task.
 */
export function declarationsDrift(
  partial: Partial<SheetDeclarations> | undefined,
): string[] {
  if (!partial) return [];
  const all = new Set((partial.all ?? []).map((value) => value.id));
  // No `all` at all is the documented "old behaviour" case — nothing was
  // published, so nothing went unclaimed, and there is nothing to disagree.
  if (partial.all === undefined) return [];

  const missing: string[] = [];
  for (const set of NAMED_DECLARATION_SETS) {
    for (const value of partial[set] ?? []) {
      if (!all.has(value.id) && !missing.includes(value.id)) {
        missing.push(value.id);
      }
    }
  }
  return missing;
}

/** Sets already complained about, so drift costs one line and not one per render. */
const driftWarned = new Set<string>();

/** Test seam: forget what has already been warned about. */
export function resetDeclarationDriftWarnings(): void {
  driftWarned.clear();
}

function reportDrift(
  partial: Partial<SheetDeclarations>,
  _claimed: ReadonlySet<string>,
): void {
  const missing = declarationsDrift(partial);
  if (missing.length === 0) return;
  const key = missing.join(",");
  if (driftWarned.has(key)) return;
  driftWarned.add(key);
  console.warn(
    `[layout] declared sets disagree with \`all\`: ${key} — \`other\` is computed from \`all\`, so values in no named set may be missing from the sheet`,
  );
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
