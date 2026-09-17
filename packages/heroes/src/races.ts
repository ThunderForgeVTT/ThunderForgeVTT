/**
 * What a race looks like, to the dice (spec 044 FR-007a, research R6).
 *
 * A race is presentation data, not a rule: it lists only what makes it
 * recognisable — an elf's ears, a dwarf's beard — and leaves everything else
 * to the roll, so a hundred elves are a hundred elves. Every look is built
 * from parts that already exist; a race that needs a part we do not draw
 * waits for the part.
 *
 * A race is never stored. It narrows a roll and nothing else (contract B5a):
 * the spec a roll produces has no `race` field, and nothing here writes a
 * character sheet.
 */
import {
  HERO_COLORS,
  HERO_FLAGS,
  HERO_PARTS,
  MONSTER_TONES,
  SKIN_TONES,
  type ResolvedHero,
  type SizeCategory,
} from "./spec.ts";

export type RaceKey = string;

type PartField = keyof typeof HERO_PARTS;
type ColorField = (typeof HERO_COLORS)[number];
type FlagField = (typeof HERO_FLAGS)[number];

export interface RaceLook {
  /** Lower-case names a character sheet might use; unique across races. */
  aliases: readonly string[];
  /** Fields whose roll is narrowed to these choices. */
  choices?: Partial<Record<PartField, readonly string[]>> & {
    size?: readonly SizeCategory[];
  };
  /** Colour fields whose roll is narrowed to these swatches. */
  swatches?: Partial<Record<ColorField, readonly string[]>>;
  /** Flags a race always has on, or always off. */
  flags?: Partial<Record<FlagField, boolean>>;
}

const NATURAL = [
  SKIN_TONES.porcelain,
  SKIN_TONES.peach,
  SKIN_TONES.honey,
  SKIN_TONES.bronze,
  SKIN_TONES.umber,
  SKIN_TONES.ebony,
];

export const HERO_RACES: Readonly<Record<RaceKey, RaceLook>> = {
  human: {
    aliases: ["humans"],
    choices: { ears: ["round"] },
    swatches: { skin: NATURAL },
  },
  elf: {
    aliases: ["high elf", "wood elf", "dark elf", "drow", "eladrin", "elves"],
    choices: { ears: ["pointed"] },
    flags: { beard: false },
  },
  "half-elf": {
    aliases: ["half elf", "halfelf"],
    choices: { ears: ["pointed"] },
  },
  dwarf: {
    aliases: ["hill dwarf", "mountain dwarf", "duergar", "dwarves"],
    choices: { ears: ["round"], size: ["medium"] },
    flags: { beard: true },
  },
  halfling: {
    aliases: ["lightfoot", "stout", "lightfoot halfling", "stout halfling"],
    choices: { ears: ["round"], size: ["small"] },
  },
  gnome: {
    aliases: ["forest gnome", "rock gnome", "deep gnome", "svirfneblin"],
    choices: { ears: ["pointed"], size: ["small"] },
  },
  orc: {
    aliases: ["orcs"],
    swatches: { skin: [SKIN_TONES.sage, MONSTER_TONES.orcMoss] },
    flags: { tusks: true },
  },
  "half-orc": {
    aliases: ["half orc", "halforc"],
    swatches: { skin: [SKIN_TONES.sage, MONSTER_TONES.orcMoss, ...NATURAL] },
    flags: { tusks: true },
  },
  tiefling: {
    aliases: ["tieflings"],
    choices: { headgear: ["tiefling"] },
    swatches: {
      skin: [SKIN_TONES.ember, MONSTER_TONES.fiendCrimson, ...NATURAL],
    },
  },
  dragonborn: {
    aliases: ["dragonkin"],
    choices: { muzzle: ["snout"], hide: ["scales"] },
    swatches: {
      skin: [
        MONSTER_TONES.dragonScarlet,
        MONSTER_TONES.dragonEmerald,
        MONSTER_TONES.dragonSapphire,
      ],
    },
  },
  goblin: {
    aliases: ["goblins"],
    choices: { ears: ["pointed"], size: ["small"] },
    swatches: { skin: [MONSTER_TONES.goblinGreen] },
  },
};

/** The race a sheet's free text names, or null. Trims and ignores case, and
 * matches a race's key or any of its aliases: "High Elf" → "elf",
 * "Moonkin" → null. */
export function matchRace(text: string | null | undefined): RaceKey | null {
  if (typeof text !== "string") return null;
  const wanted = text.trim().toLowerCase().replace(/\s+/g, " ");
  if (wanted === "") return null;
  for (const [key, look] of Object.entries(HERO_RACES)) {
    if (key === wanted || look.aliases.includes(wanted)) return key;
  }
  return null;
}

/** How `hero` breaks `race`'s look, one line per field — empty when it fits.
 * The package's tests and the builder's e2e both hold rolls to this. */
export function raceProblems(hero: ResolvedHero, race: RaceKey): string[] {
  const look = HERO_RACES[race];
  if (!look) return [`${race}: not a race`];
  const problems: string[] = [];
  const record = hero as unknown as Record<string, unknown>;
  for (const [field, allowed] of Object.entries(look.choices ?? {})) {
    if (!(allowed as readonly string[]).includes(String(record[field]))) {
      problems.push(`${field}: ${race} is one of ${allowed.join(", ")}`);
    }
  }
  for (const [field, allowed] of Object.entries(look.swatches ?? {})) {
    if (!(allowed as readonly string[]).includes(String(record[field]))) {
      problems.push(`${field}: ${race} is one of ${allowed.join(", ")}`);
    }
  }
  for (const [field, value] of Object.entries(look.flags ?? {})) {
    if (record[field] !== value) problems.push(`${field}: ${race} is ${value}`);
  }
  return problems;
}
