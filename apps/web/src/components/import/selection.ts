/**
 * What a Game Master decided about a book they have just read.
 *
 * The review is a decision, not an edit: the read itself is never mutated, and
 * what submit hands back is derived from the read plus these three decisions —
 * kinds left out, entries left out, and uncertain values corrected by hand
 * (049 FR-026, and spec 048 FR-022 for the corrections). Keeping the decisions
 * separate from the entries is what makes FR-027 checkable — what is committed
 * is what the review showed — because there is exactly one function that turns
 * one into the other, and a test can hold it to that.
 *
 * An entry is identified by its position in the read. `ContentEntry` carries no
 * identifier of its own: two printings of the same spell in one book are two
 * entries with the same name and the same fields, and a key built out of the
 * content would fold them into one.
 */
import type { ContentEntry } from "@/engine/sdk/ContentEntry";

/** A decision in progress. Nothing here is excluded until somebody says so. */
export interface Review {
  readonly excludedKinds: ReadonlySet<string>;
  readonly excludedEntries: ReadonlySet<number>;
  /** Position in the read, then field key, then what the Game Master read. */
  readonly corrections: ReadonlyMap<number, ReadonlyMap<string, string>>;
}

export function nothingExcluded(): Review {
  return {
    excludedKinds: new Set(),
    excludedEntries: new Set(),
    corrections: new Map(),
  };
}

function toggled<T>(set: ReadonlySet<T>, member: T, present: boolean) {
  const next = new Set(set);
  if (present) {
    next.add(member);
  } else {
    next.delete(member);
  }
  return next;
}

/** Include or exclude a whole kind — every spell, every creature (FR-026). */
export function includeKind(
  review: Review,
  kind: string,
  included: boolean,
): Review {
  return {
    ...review,
    excludedKinds: toggled(review.excludedKinds, kind, !included),
  };
}

/** Include or exclude one entry (FR-026). */
export function includeEntry(
  review: Review,
  at: number,
  included: boolean,
): Review {
  return {
    ...review,
    excludedEntries: toggled(review.excludedEntries, at, !included),
  };
}

/**
 * Record what a Game Master read off the page for an uncertain field.
 *
 * Blanking the box withdraws the correction rather than recording an empty
 * value: a reviewer who changes their mind should get the reader's own reading
 * back, and "the Game Master says this field is empty" is a claim the review
 * has no way to make honestly.
 */
export function correct(
  review: Review,
  at: number,
  field: string,
  value: string,
): Review {
  const corrections = new Map(review.corrections);
  const forEntry = new Map(corrections.get(at) ?? []);
  if (value.trim() === "") {
    forEntry.delete(field);
  } else {
    forEntry.set(field, value);
  }
  if (forEntry.size === 0) {
    corrections.delete(at);
  } else {
    corrections.set(at, forEntry);
  }
  return { ...review, corrections };
}

/** What a Game Master has typed into a field's box, if anything. */
export function correctionFor(
  review: Review,
  at: number,
  field: string,
): string | undefined {
  return review.corrections.get(at)?.get(field);
}

/** Whether this entry would be submitted as things stand. */
export function isIncluded(
  review: Review,
  entries: readonly ContentEntry[],
  at: number,
): boolean {
  const entry = entries[at];
  if (!entry) return false;
  return (
    !review.excludedKinds.has(entry.kind) && !review.excludedEntries.has(at)
  );
}

/**
 * The entries the Game Master approved, with their corrections applied.
 *
 * The single crossing from "what the review showed" to "what is submitted"
 * (FR-027). A correction only lands on a field the reader was *uncertain*
 * about: a clear reading is not up for revision here, and an unread field has
 * no value to revise — inventing one is the thing FR-002 exists to forbid.
 *
 * A corrected field becomes `clear`, because a person has just read it off the
 * page, which is the best evidence this system will ever have about it.
 */
export function approved(
  entries: readonly ContentEntry[],
  review: Review,
): ContentEntry[] {
  const kept: ContentEntry[] = [];
  entries.forEach((entry, at) => {
    if (!isIncluded(review, entries, at)) return;
    const corrections = review.corrections.get(at);
    if (!corrections || corrections.size === 0) {
      kept.push(entry);
      return;
    }
    const values = { ...entry.values };
    for (const [field, value] of corrections) {
      if (values[field]?.state !== "uncertain") continue;
      values[field] = { state: "clear", value };
    }
    kept.push({ ...entry, values });
  });
  return kept;
}

/** One kind of content, how much of it was found, and how much survives. */
export interface KindTally {
  kind: string;
  found: number;
  included: number;
}

/** Per kind, and in the order the kinds first appeared in the book (FR-021). */
export function tally(
  entries: readonly ContentEntry[],
  review: Review,
): KindTally[] {
  const tallies = new Map<string, KindTally>();
  entries.forEach((entry, at) => {
    const existing = tallies.get(entry.kind) ?? {
      kind: entry.kind,
      found: 0,
      included: 0,
    };
    existing.found += 1;
    if (isIncluded(review, entries, at)) existing.included += 1;
    tallies.set(entry.kind, existing);
  });
  return [...tallies.values()];
}
