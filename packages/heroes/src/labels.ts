/**
 * What a person reads for every field and choice a hero builder offers.
 *
 * Kept here rather than in the builder so a choice added to `spec.ts` without
 * a label fails this package's check, not a screen three layers away: the
 * builder shows a key where a label is missing (FR-002), and `labelProblems`
 * is what makes that never ship.
 */
import { HERO_RACES, type RaceKey } from "./races.ts";
import {
  HERO_COLORS,
  HERO_FLAGS,
  HERO_PARTS,
  SIZES,
  type SizeCategory,
} from "./spec.ts";

export interface HeroLabels {
  /** Every field a builder draws a control for. */
  fields: Record<string, string>;
  /** field → choice → label, for every part and for `size`. */
  choices: Record<string, Record<string, string>>;
  races: Record<RaceKey, string>;
}

export const HERO_LABELS: HeroLabels = {
  fields: {
    name: "Name",
    title: "Title",
    size: "Size",
    race: "Race",
    ears: "Ears",
    mouth: "Mouth",
    hair: "Hair",
    headgear: "Headgear",
    emblem: "Emblem",
    prop: "Held item",
    build: "Build",
    muzzle: "Muzzle",
    eyeStyle: "Eyes",
    hide: "Hide",
    wings: "Wings",
    tail: "Tail",
    skin: "Skin",
    eyes: "Eye colour",
    hairColor: "Hair colour",
    beardColor: "Beard colour",
    headgearColor: "Headgear colour",
    accent: "Accent",
    outfit: "Outfit",
    trim: "Trim",
    glow: "Backdrop",
    ring: "Token rim",
    hideColor: "Hide colour",
    wingColor: "Wing colour",
    tusks: "Tusks",
    beard: "Beard",
  },
  choices: {
    ears: { round: "Round", pointed: "Pointed" },
    mouth: {
      smile: "Smile",
      grin: "Grin",
      smirk: "Smirk",
      snarl: "Snarl",
      gape: "Gape",
    },
    hair: {
      bald: "Bald",
      short: "Short",
      long: "Long",
      braids: "Braids",
      bun: "Bun",
      spiky: "Spiky",
      curly: "Curly",
    },
    headgear: {
      none: "None",
      helm: "Helm",
      wizard: "Wizard's hat",
      hood: "Hood",
      circlet: "Circlet",
      horned: "Horned helm",
      cap: "Cap",
      leaves: "Leaf crown",
      headband: "Headband",
      goggles: "Goggles",
      tiefling: "Horns",
      crest: "Crest",
      antlers: "Antlers",
      boneCrown: "Bone crown",
    },
    emblem: { none: "None", sun: "Sun", cross: "Cross", gear: "Gear" },
    prop: {
      none: "Nothing",
      sword: "Sword",
      staff: "Staff",
      dagger: "Dagger",
      bow: "Bow",
      lute: "Lute",
      axe: "Axe",
      vine: "Vine",
      hammer: "Hammer",
      fists: "Fists",
      flame: "Flame",
      wrench: "Wrench",
      club: "Club",
      claws: "Claws",
    },
    build: {
      upright: "Upright",
      hulking: "Hulking",
      hunched: "Hunched",
      sinuous: "Sinuous",
    },
    muzzle: { none: "None", snout: "Snout", maw: "Maw", beak: "Beak" },
    eyeStyle: {
      round: "Round",
      slit: "Slit",
      beady: "Beady",
      hollow: "Hollow",
    },
    hide: {
      none: "None",
      scales: "Scales",
      fur: "Fur",
      bone: "Bone",
      warts: "Warts",
    },
    wings: { none: "None", bat: "Bat", feathered: "Feathered" },
    tail: { none: "None", reptile: "Reptile", spiked: "Spiked", tuft: "Tuft" },
    size: {
      tiny: "Tiny",
      small: "Small",
      medium: "Medium",
      large: "Large",
      huge: "Huge",
      gargantuan: "Gargantuan",
    } satisfies Record<SizeCategory, string>,
  },
  races: {
    human: "Human",
    elf: "Elf",
    "half-elf": "Half-elf",
    dwarf: "Dwarf",
    halfling: "Halfling",
    gnome: "Gnome",
    orc: "Orc",
    "half-orc": "Half-orc",
    tiefling: "Tiefling",
    dragonborn: "Dragonborn",
    goblin: "Goblin",
  },
};

/** Every field, choice or race `labels` has no text for, as `field` or
 * `field.choice` — empty when the catalogue is fully labelled. */
export function labelProblems(labels: HeroLabels = HERO_LABELS): string[] {
  const problems: string[] = [];
  const fields = [
    "name",
    "title",
    "size",
    "race",
    ...Object.keys(HERO_PARTS),
    ...HERO_COLORS,
    ...HERO_FLAGS,
  ];
  for (const field of fields) {
    if (!labels.fields[field]) problems.push(field);
  }
  const choiceLists: [string, readonly string[]][] = [
    ...Object.entries(HERO_PARTS),
    ["size", SIZES],
  ];
  for (const [field, options] of choiceLists) {
    for (const option of options) {
      if (!labels.choices[field]?.[option]) {
        problems.push(`${field}.${option}`);
      }
    }
  }
  for (const race of Object.keys(HERO_RACES)) {
    if (!labels.races[race]) problems.push(`race.${race}`);
  }
  return problems;
}
