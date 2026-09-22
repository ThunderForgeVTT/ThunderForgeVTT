/**
 * What a hero is made of — the spec a portrait and a token are drawn from.
 *
 * Every choice is a closed list, and the lists are exported: a hero builder
 * renders its controls from `HERO_PARTS` and `HERO_COLORS` rather than from
 * its own copy, so a part added here appears in the builder without it
 * learning the part's name.
 *
 * Monsters are heroes too. A goblin is a small green person, a troll is a
 * hunched one and a dragon is a maw, a crest and a pair of wings on the same
 * skeleton: every creature part below joins the same list of choices rather
 * than starting a second vocabulary, so the builder, the validator and the
 * "no two choices draw alike" test cover monsters the day they are added.
 */
import { HEX_COLOR, shade } from "./color.ts";

export const EAR_SHAPES = ["round", "pointed"] as const;
export const MOUTHS = ["smile", "grin", "smirk", "snarl", "gape"] as const;
export const HAIR_STYLES = [
  "bald",
  "short",
  "long",
  "braids",
  "bun",
  "spiky",
  "curly",
] as const;
export const HEADGEAR = [
  "none",
  "helm",
  "wizard",
  "hood",
  "circlet",
  "horned",
  "cap",
  "leaves",
  "headband",
  "goggles",
  "tiefling",
  "crest",
  "antlers",
  "boneCrown",
] as const;
export const EMBLEMS = ["none", "sun", "cross", "gear"] as const;
export const PROPS = [
  "none",
  "sword",
  "staff",
  "dagger",
  "bow",
  "lute",
  "axe",
  "vine",
  "hammer",
  "fists",
  "flame",
  "wrench",
  "club",
  "claws",
] as const;

/** How the body is carried. The head parts all draw at fixed coordinates, so
 * a build moves the whole head rather than each part knowing the pose: a
 * troll's hunch and a dragon's raised neck are one transform and one
 * silhouette apart. */
export const BUILDS = ["upright", "hulking", "hunched", "sinuous"] as const;
/** What replaces a flat face. A muzzle draws its own mouth, so `mouth` is
 * ignored while one is worn. */
export const MUZZLES = ["none", "snout", "maw", "beak"] as const;
export const EYE_STYLES = ["round", "slit", "beady", "hollow"] as const;
/** What the skin is covered in, drawn over both the face and the chest so a
 * scaled creature is scaled all the way down. */
export const HIDES = ["none", "scales", "fur", "bone", "warts"] as const;
export const WINGS = ["none", "bat", "feathered"] as const;
export const TAILS = ["none", "reptile", "spiked", "tuft"] as const;

export type EarShape = (typeof EAR_SHAPES)[number];
export type Mouth = (typeof MOUTHS)[number];
export type HairStyle = (typeof HAIR_STYLES)[number];
export type Headgear = (typeof HEADGEAR)[number];
export type Emblem = (typeof EMBLEMS)[number];
export type Prop = (typeof PROPS)[number];
export type Build = (typeof BUILDS)[number];
export type Muzzle = (typeof MUZZLES)[number];
export type EyeStyle = (typeof EYE_STYLES)[number];
export type Hide = (typeof HIDES)[number];
export type Wings = (typeof WINGS)[number];
export type Tail = (typeof TAILS)[number];

/** Every part with a fixed set of choices, keyed by the spec field it sets. */
export const HERO_PARTS = {
  ears: EAR_SHAPES,
  mouth: MOUTHS,
  hair: HAIR_STYLES,
  headgear: HEADGEAR,
  emblem: EMBLEMS,
  prop: PROPS,
  build: BUILDS,
  muzzle: MUZZLES,
  eyeStyle: EYE_STYLES,
  hide: HIDES,
  wings: WINGS,
  tail: TAILS,
} as const;

/** How many grid cells across the creature stands, by size category.
 *
 * The ladder is the familiar tabletop one; the numbers are cells, not any
 * one system's scale factor, because a system pack declares its own
 * categories (Genie calls them diminutive to colossal) and maps them onto
 * these. Tiny is half a cell, which is the engine's `MIN_FOOTPRINT`: nothing
 * on the board may be smaller, so a spec that asked for less would only be
 * clamped later and confuse whoever compared the two numbers. */
export const SIZE_CATEGORIES = {
  tiny: 0.5,
  small: 1,
  medium: 1,
  large: 2,
  huge: 3,
  gargantuan: 4,
} as const;

export const SIZES = Object.keys(SIZE_CATEGORIES) as readonly SizeCategory[];

export type SizeCategory = keyof typeof SIZE_CATEGORIES;

/** Every spec field that is a colour. */
export const HERO_COLORS = [
  "skin",
  "eyes",
  "hairColor",
  "beardColor",
  "headgearColor",
  "accent",
  "outfit",
  "trim",
  "glow",
  "ring",
  "hideColor",
  "wingColor",
] as const;

/** Every spec field that is on or off. */
export const HERO_FLAGS = ["tusks", "beard"] as const;

/** Skin tones the presets use; a builder's starting swatches. Any `#rrggbb`
 * is a valid skin. */
export const SKIN_TONES = {
  porcelain: "#f7dcc7",
  peach: "#f1c6a0",
  honey: "#d9a066",
  bronze: "#b87a4b",
  umber: "#8a5635",
  ebony: "#5e3a24",
  sage: "#8fbf6a",
  ember: "#d9574a",
} as const;

/** Hides a monster wears. Kept beside `SKIN_TONES` rather than inside the
 * bestiary so a builder offers the same swatches for a troll that the
 * generator reaches for. */
export const MONSTER_TONES = {
  goblinGreen: "#7bb04a",
  orcMoss: "#5f8f52",
  trollStone: "#8f9e7a",
  ogreClay: "#c99a6a",
  boneWhite: "#e8e2d0",
  graveGrey: "#9aa79a",
  dragonScarlet: "#c0392b",
  dragonEmerald: "#2f8f5b",
  dragonSapphire: "#3a6fb0",
  fiendCrimson: "#8f2f3a",
  beastBrown: "#8a6242",
  oozeViolet: "#7a5aa8",
} as const;

/** A hero with every field decided — what the parts draw from. */
export interface ResolvedHero {
  name: string;
  /** "the Wizard"; empty when the hero has none. */
  title: string;
  skin: string;
  eyes: string;
  ears: EarShape;
  mouth: Mouth;
  tusks: boolean;
  hair: HairStyle;
  hairColor: string;
  beard: boolean;
  beardColor: string;
  headgear: Headgear;
  headgearColor: string;
  /** The small bright detail on the headgear: a helm's or cap's plume, a
   * circlet's gem. */
  accent: string;
  outfit: string;
  trim: string;
  emblem: Emblem;
  prop: Prop;
  /** The backdrop behind the hero. */
  glow: string;
  /** The token's rim. */
  ring: string;
  build: Build;
  muzzle: Muzzle;
  eyeStyle: EyeStyle;
  hide: Hide;
  /** Scales, fur tufts, ribs — the marks drawn on top of the skin. */
  hideColor: string;
  wings: Wings;
  wingColor: string;
  tail: Tail;
  /** How many cells the creature stands across. Carried on the spec rather
   * than worked out by whoever places it, because a monster that is drawn
   * huge and placed in one square is the bug spec 047 FR-020 is about. */
  size: SizeCategory;
}

/** A hero as written: a name, and whatever else differs from the defaults. */
export type HeroSpec = { name: string } & Partial<Omit<ResolvedHero, "name">>;

/** The longest name and title `validateHero` accepts, in UTF-16 code units
 * (`String.length`). Exported so `HERO_SPEC_SCHEMA` states the same bounds
 * rather than a copy of them. */
export const HERO_TEXT_LIMITS = { name: 80, title: 120 } as const;
const MAX_NAME = HERO_TEXT_LIMITS.name;
const MAX_TITLE = HERO_TEXT_LIMITS.title;

const ACCENT_BY_HEADGEAR: Partial<Record<Headgear, string>> = {
  helm: "#d94a4a",
  circlet: "#4ec3e0",
};

export type HeroValidation =
  | { ok: true; hero: ResolvedHero }
  | { ok: false; problems: string[] };

/** Every problem with a hero spec, named by field, or the hero with its
 * defaults filled in. Takes `unknown` because a spec arrives from saved JSON
 * as often as from code; an unknown field is a problem, not ignored, so a
 * misspelt part is not silently drawn as the default. */
export function validateHero(input: unknown): HeroValidation {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    return { ok: false, problems: ["a hero is an object"] };
  }
  const raw = input as Record<string, unknown>;
  const problems: string[] = [];
  const known = new Set<string>([
    "name",
    "title",
    "size",
    ...Object.keys(HERO_PARTS),
    ...HERO_COLORS,
    ...HERO_FLAGS,
  ]);
  for (const key of Object.keys(raw)) {
    if (!known.has(key)) problems.push(`${key}: not a hero field`);
  }

  const text = (key: string, max: number, fallback?: string): string => {
    const value = raw[key];
    if (value === undefined && fallback !== undefined) return fallback;
    if (typeof value !== "string" || value.trim() === "") {
      problems.push(`${key}: needs some text`);
      return "";
    }
    if (value.length > max) {
      problems.push(`${key}: at most ${max} characters`);
    }
    return value.trim();
  };
  const color = (key: string, fallback: string): string => {
    const value = raw[key];
    if (value === undefined) return fallback;
    if (typeof value === "string" && HEX_COLOR.test(value)) {
      return value.toLowerCase();
    }
    problems.push(`${key}: a colour is written #rrggbb`);
    return fallback;
  };
  const choice = <T extends string>(
    key: string,
    options: readonly T[],
    fallback: T,
  ): T => {
    const value = raw[key];
    if (value === undefined) return fallback;
    if (
      typeof value === "string" &&
      (options as readonly string[]).includes(value)
    ) {
      return value as T;
    }
    problems.push(`${key}: one of ${options.join(", ")}`);
    return fallback;
  };
  const flag = (key: string): boolean => {
    const value = raw[key];
    if (value === undefined) return false;
    if (typeof value === "boolean") return value;
    problems.push(`${key}: true or false`);
    return false;
  };

  const hairColor = color("hairColor", "#3b2a1f");
  const headgear = choice("headgear", HEADGEAR, "none");
  const outfit = color("outfit", "#3d6fd1");
  // Hide and wings default off the skin so a creature given nothing but a
  // colour still reads as one animal rather than a body with somebody else's
  // wings attached.
  const skin = color("skin", SKIN_TONES.peach);
  const hero: ResolvedHero = {
    name: text("name", MAX_NAME),
    title: raw.title === "" ? "" : text("title", MAX_TITLE, ""),
    skin,
    eyes: color("eyes", "#3b2a1f"),
    ears: choice("ears", EAR_SHAPES, "round"),
    mouth: choice("mouth", MOUTHS, "smile"),
    tusks: flag("tusks"),
    hair: choice("hair", HAIR_STYLES, "short"),
    hairColor,
    beard: flag("beard"),
    beardColor: color("beardColor", hairColor),
    headgear,
    headgearColor: color("headgearColor", "#b8c2cc"),
    accent: color("accent", ACCENT_BY_HEADGEAR[headgear] ?? "#f4c542"),
    outfit,
    trim: color("trim", shade(outfit, 0.35)),
    emblem: choice("emblem", EMBLEMS, "none"),
    prop: choice("prop", PROPS, "none"),
    glow: color("glow", "#c9d6e8"),
    ring: color("ring", outfit),
    build: choice("build", BUILDS, "upright"),
    muzzle: choice("muzzle", MUZZLES, "none"),
    eyeStyle: choice("eyeStyle", EYE_STYLES, "round"),
    hide: choice("hide", HIDES, "none"),
    hideColor: color("hideColor", shade(skin, 0.3)),
    wings: choice("wings", WINGS, "none"),
    wingColor: color("wingColor", shade(skin, 0.15)),
    tail: choice("tail", TAILS, "none"),
    size: choice("size", SIZES, "medium"),
  };
  return problems.length > 0 ? { ok: false, problems } : { ok: true, hero };
}

/** Thrown by the factory for a spec that is not a hero. */
export class HeroSpecError extends Error {
  readonly problems: readonly string[];

  constructor(problems: readonly string[]) {
    super(`not a valid hero: ${problems.join("; ")}`);
    this.name = "HeroSpecError";
    this.problems = problems;
  }
}

/** The hero `spec` describes, defaults filled in; throws `HeroSpecError`. */
export function resolveHero(spec: HeroSpec): ResolvedHero {
  const result = validateHero(spec);
  if (!result.ok) throw new HeroSpecError(result.problems);
  return result.hero;
}
