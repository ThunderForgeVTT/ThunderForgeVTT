/**
 * The dice: a hero from a seed, optionally narrowed to a race (FR-007,
 * FR-007a).
 *
 * Built on `seeded`, so a seed is a hero — the builder shows the seed and a
 * Game Master can type it back in. The race is folded into the seed, so
 * `(seed, race)` is reproducible too.
 *
 * The roll makes a person, not a monster. Creature parts — build, muzzle,
 * hide, wings, tail, eye style — and size stay at their defaults unless a
 * race asks for them (a dragonborn's snout and scales); a Game Master who
 * wants a winged goblin sets the wings by hand. Colours that follow another
 * colour by default (trim, ring, beard, accent, hide, wings) are not rolled,
 * so they keep following.
 */
import { HERO_PALETTES } from "./palettes.ts";
import { HERO_RACES, type RaceKey } from "./races.ts";
import { seeded } from "./seed.ts";
import { HERO_PARTS, type HeroSpec, type ResolvedHero } from "./spec.ts";

/** A rolled hero: every field of a spec except the name, which is the
 * caller's. */
export type RolledHero = Omit<HeroSpec, "name">;

export interface RollOptions {
  /** Fields the roll leaves alone; they are absent from the result so the
   * caller's own value survives a merge. A lock beats a race. */
  locked?: Partial<Record<keyof ResolvedHero, true>>;
  /** Narrow the roll to this race's look. Absent means "any". */
  race?: RaceKey | null;
}

const PERSON_PARTS = [
  "ears",
  "mouth",
  "hair",
  "headgear",
  "emblem",
  "prop",
] as const satisfies readonly (keyof typeof HERO_PARTS)[];

const ROLLED_COLORS = [
  "skin",
  "eyes",
  "hairColor",
  "headgearColor",
  "outfit",
  "glow",
] as const;

export function randomHero(
  seed: string,
  options: RollOptions = {},
): RolledHero {
  const race = options.race ?? null;
  const look = race === null ? undefined : HERO_RACES[race];
  if (race !== null && !look) throw new Error(`${race}: not a race`);
  const dice = seeded(`${seed}|${race ?? "any"}`);
  const locked = options.locked ?? {};
  const rolled: Record<string, unknown> = {};
  const set = (field: string, value: unknown) => {
    if (!locked[field as keyof ResolvedHero]) rolled[field] = value;
  };

  const choices = look?.choices ?? {};
  const parts = new Set<string>([...PERSON_PARTS, ...Object.keys(choices)]);
  for (const field of parts) {
    const narrowed = (choices as Record<string, readonly string[]>)[field];
    const all =
      field === "size"
        ? undefined
        : HERO_PARTS[field as keyof typeof HERO_PARTS];
    const from = narrowed ?? all;
    if (from) set(field, dice.pick(field, from));
  }

  const swatches = look?.swatches ?? {};
  const colors = new Set<string>([...ROLLED_COLORS, ...Object.keys(swatches)]);
  for (const field of colors) {
    const from =
      (swatches as Record<string, readonly string[]>)[field] ??
      HERO_PALETTES[field as keyof typeof HERO_PALETTES];
    set(field, dice.pick(field, from));
  }

  const flags = look?.flags ?? {};
  set("beard", flags.beard ?? dice.pick("beard", [false, false, true]));
  if (flags.tusks !== undefined) set("tusks", flags.tusks);

  return rolled as RolledHero;
}
