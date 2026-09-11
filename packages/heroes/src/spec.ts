/**
 * What a hero is made of — the spec a portrait and a token are drawn from.
 *
 * Every choice is a closed list, and the lists are exported: a hero builder
 * renders its controls from `HERO_PARTS` and `HERO_COLORS` rather than from
 * its own copy, so a part added here appears in the builder without it
 * learning the part's name.
 */
import { HEX_COLOR, shade } from "./color.ts";

export const EAR_SHAPES = ["round", "pointed"] as const;
export const MOUTHS = ["smile", "grin", "smirk"] as const;
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
] as const;

export type EarShape = (typeof EAR_SHAPES)[number];
export type Mouth = (typeof MOUTHS)[number];
export type HairStyle = (typeof HAIR_STYLES)[number];
export type Headgear = (typeof HEADGEAR)[number];
export type Emblem = (typeof EMBLEMS)[number];
export type Prop = (typeof PROPS)[number];

/** Every part with a fixed set of choices, keyed by the spec field it sets. */
export const HERO_PARTS = {
  ears: EAR_SHAPES,
  mouth: MOUTHS,
  hair: HAIR_STYLES,
  headgear: HEADGEAR,
  emblem: EMBLEMS,
  prop: PROPS,
} as const;

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
}

/** A hero as written: a name, and whatever else differs from the defaults. */
export type HeroSpec = { name: string } & Partial<Omit<ResolvedHero, "name">>;

const MAX_NAME = 80;
const MAX_TITLE = 120;

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
  const hero: ResolvedHero = {
    name: text("name", MAX_NAME),
    title: raw.title === "" ? "" : text("title", MAX_TITLE, ""),
    skin: color("skin", SKIN_TONES.peach),
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
