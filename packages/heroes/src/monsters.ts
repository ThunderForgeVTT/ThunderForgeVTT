/**
 * A monster from as little as a name.
 *
 * The input this is built for is a statblock read out of a book
 * (`thunderforge-pdf` into the 5e pack's reader): a name, a descriptor line
 * like "Gargantuan dragon, chaotic evil", and no art at all. That is all this
 * needs, and it degrades the whole way down — an unrecognised name still has
 * a creature type, an unrecognised type still has a size, and a bare name
 * still draws something plausible rather than nothing.
 *
 * Everything is decided from the seed, which is the creature's name unless a
 * caller says otherwise. A monster that looked different this week from last
 * week would be a bug at the table, not a feature.
 */
import {
  CREATURES,
  FAMILIES,
  TINTS,
  type CreatureEntry,
  type CreatureName,
  type FamilyName,
  type Varies,
} from "./families.ts";
import { createHero, type Hero } from "./render.ts";
import { seeded } from "./seed.ts";
import { SIZES, type HeroSpec, type SizeCategory } from "./spec.ts";

/** A creature as a source describes it, which is never very much. */
export interface CreatureSource {
  name: string;
  /** The descriptor line as printed — "Gargantuan dragon, chaotic evil" — or
   * just the creature type word. Both are read the same way. */
  descriptor?: string;
  /** The size word, when the caller has it apart from the descriptor. Wins
   * over the descriptor, because a caller that passes it has already read the
   * book more carefully than this can. */
  size?: string;
  /** Defaults to the name, so the same creature is always the same creature.
   * Pass a different one for the second goblin in a pack. */
  seed?: string;
}

/** What the source was understood to be. Worth having separately from the
 * drawing: a Game Master looking at a bestiary import wants to see that
 * "Ancient Brass Dragon" was read as a Gargantuan dragon, and a wrong
 * reading is far easier to spot stated in words than inferred from a
 * picture. */
export interface CreatureReading {
  family: FamilyName;
  /** The named creature it matched, or null when only its type was known. */
  creature: CreatureName | null;
  size: SizeCategory;
  /** The colour its name gave it, if any. */
  tint: string | null;
}

function words(text: string): string[] {
  return text
    .toLowerCase()
    .split(/[^a-z]+/i)
    .filter(Boolean);
}

/** The longest key whose start a word matches.
 *
 * Longest first so "hobgoblin" is a hobgoblin; `startsWith` so "goblins",
 * "wolves" — near enough — and "dragon," all land, which matters because
 * book text arrives with its punctuation and its plurals attached. */
function match(
  keys: readonly string[],
  from: readonly string[],
): string | null {
  const byLength = [...keys].sort((a, b) => b.length - a.length);
  for (const key of byLength) {
    if (from.some((word) => word.startsWith(key))) return key;
  }
  return null;
}

function sizeOf(text: string | undefined): SizeCategory | null {
  if (text === undefined) return null;
  const found = match(SIZES, words(text));
  return found === null ? null : (found as SizeCategory);
}

export function readCreature(source: CreatureSource): CreatureReading {
  const nameWords = words(source.name);
  const creature = match(
    Object.keys(CREATURES),
    nameWords,
  ) as CreatureName | null;
  const entry: CreatureEntry | null =
    creature === null ? null : CREATURES[creature];
  // A named creature's family beats the book's type word: the 5e books call a
  // gnoll a humanoid, and drawing one as a person with a spear rather than as
  // the hyena it is would be obeying the letter of the statblock over the
  // point of the picture. Failing both, the name itself often says the type
  // outright — "Fire Giant", "Air Elemental" — and reading it there is the
  // difference between a giant and a shrug.
  const family: FamilyName = (entry?.family ??
    match(Object.keys(FAMILIES), words(source.descriptor ?? "")) ??
    match(Object.keys(FAMILIES), nameWords) ??
    "monstrosity") as FamilyName;
  const size =
    sizeOf(source.size) ??
    sizeOf(source.descriptor) ??
    entry?.size ??
    FAMILIES[family].size;
  const tint = match(Object.keys(TINTS), nameWords);
  return { family, creature, size, tint: tint === null ? null : TINTS[tint]! };
}

/** The spec for the creature a source describes. Deterministic in the seed,
 * and always a valid hero spec: everything it produces comes from the closed
 * lists in `spec.ts`. */
export function monsterSpec(source: CreatureSource): HeroSpec {
  const reading = readCreature(source);
  const entry: CreatureEntry | null =
    reading.creature === null ? null : CREATURES[reading.creature];
  const family = FAMILIES[reading.family];
  const varies: Varies = { ...family.sometimes, ...entry?.sometimes };
  const choices = seeded(source.seed ?? source.name);

  const spec: Record<string, unknown> = {};
  for (const field of Object.keys(varies)) {
    // `Varies` is typed per field, so each list is a different type and the
    // loop cannot be; the spec it builds goes through `validateHero` before
    // anything draws it, which is where the type is really enforced.
    const options: readonly unknown[] | undefined =
      varies[field as keyof Varies];
    if (options !== undefined) spec[field] = choices.pick(field, options);
  }
  Object.assign(spec, family.always, entry?.always);
  // The tint is the last word on colour, because it came from the creature's
  // own name and everything before it was a guess.
  if (reading.tint !== null) spec.skin = reading.tint;
  spec.name = source.name;
  spec.size = reading.size;
  return spec as HeroSpec;
}

/** The creature a source describes, drawn. */
export function createMonster(source: CreatureSource): Hero {
  return createHero(monsterSpec(source));
}

/** `count` of the same creature, each its own monster.
 *
 * Six goblins should be six goblins, so each gets its own seed derived from
 * the shared one; the pack as a whole still reproduces exactly. */
export function monsterPack(
  source: CreatureSource,
  count: number,
): readonly Hero[] {
  const seed = source.seed ?? source.name;
  return Array.from({ length: Math.max(0, Math.trunc(count)) }, (_, i) =>
    createMonster({ ...source, seed: `${seed}#${i}` }),
  );
}
