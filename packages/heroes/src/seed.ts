/**
 * Deciding things from a name rather than from chance.
 *
 * A monster generated from a statblock must look the same in tonight's
 * session as it did last week, so nothing here touches `Math.random`: a
 * creature's appearance is a pure function of its seed, and the seed is
 * usually its name.
 *
 * Each choice is hashed from the seed *and the name of the thing being
 * chosen*, rather than drawn from a stream. Two reasons, both learned the
 * hard way: a stream's early draws turn out to be correlated across seeds
 * that differ only in a trailing digit — which is exactly what a pack of six
 * goblins is, and five of the first six came out in the same helmet — and a
 * stream makes every choice depend on how many came before it, so adding a
 * colour to one family would silently restyle every creature in it.
 */

/** 32-bit FNV-1a. */
function hash(text: string): number {
  let value = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    value ^= text.charCodeAt(i);
    value = Math.imul(value, 0x01000193);
  }
  return value >>> 0;
}

/** An avalanche pass over a hash.
 *
 * FNV-1a spreads a changed byte well enough to tell strings apart, which is
 * all the id prefixes need, but not well enough that `hash % 4` is unbiased
 * for inputs as similar as "Goblin#0" and "Goblin#1". This is the standard
 * xorshift-multiply finaliser, and it is what makes a pack look like a pack. */
function mix(value: number): number {
  let x = value >>> 0;
  x ^= x >>> 16;
  x = Math.imul(x, 0x7feb352d);
  x ^= x >>> 15;
  x = Math.imul(x, 0x846ca68b);
  x ^= x >>> 16;
  return x >>> 0;
}

/** A short stable hex hash: used for the ids inside an SVG. */
export function fnv1a(text: string): string {
  return hash(text).toString(16).padStart(8, "0");
}

/** The choices a seed makes. */
export interface Chooser {
  /** The option this seed picks for `label`. Same seed and label, same
   * answer, however many other choices are made before or after it. */
  pick<T>(label: string, from: readonly T[]): T;
}

export function seeded(seed: string): Chooser {
  return {
    // An empty list is a bug in a creature's table rather than something a
    // caller can handle, so it throws instead of returning undefined and
    // letting a missing part be discovered three layers away in the SVG.
    pick<T>(label: string, from: readonly T[]): T {
      if (from.length === 0) throw new Error(`nothing to pick for ${label}`);
      return from[mix(hash(`${seed}/${label}`)) % from.length] as T;
    },
  };
}
