/**
 * Spec 048: the fields a character brings that the sheet had no place for,
 * read out of the stored slots and written back in the shapes the validators
 * in `packs/systems/dnd5e/server/src/validators_sheet.rs` accept.
 *
 * The damage types and conditions mirror `system.json` (`damageTypes`,
 * `conditions`), as `sheet-regions.ts` mirrors its tables; a test in
 * `apps/web/src/pages/world/actor/__tests__/dnd5eSheetSections.test.tsx`
 * fails when the two drift apart.
 */

type Json = Record<string, unknown>;

export const HIT_DICE = ["d6", "d8", "d10", "d12"] as const;
export type HitDie = (typeof HIT_DICE)[number];

export interface ClassEntry {
  name: string;
  subclass: string;
  level: number;
  hitDie: HitDie;
}

export const DAMAGE_TYPES = [
  "acid",
  "bludgeoning",
  "cold",
  "fire",
  "force",
  "lightning",
  "necrotic",
  "piercing",
  "poison",
  "psychic",
  "radiant",
  "slashing",
  "thunder",
] as const;

export const CONDITIONS = [
  "blinded",
  "charmed",
  "deafened",
  "exhaustion",
  "frightened",
  "grappled",
  "incapacitated",
  "invisible",
  "paralyzed",
  "petrified",
  "poisoned",
  "prone",
  "restrained",
  "stunned",
  "unconscious",
] as const;

export const DEFENCES = [
  { key: "resistances", label: "Resistances", choices: DAMAGE_TYPES },
  { key: "immunities", label: "Immunities", choices: DAMAGE_TYPES },
  { key: "vulnerabilities", label: "Vulnerabilities", choices: DAMAGE_TYPES },
  {
    key: "condition_immunities",
    label: "Condition immunities",
    choices: CONDITIONS,
  },
] as const;
export type DefenceKey = (typeof DEFENCES)[number]["key"];

export const COINS = [
  { key: "pp", label: "Platinum" },
  { key: "gp", label: "Gold" },
  { key: "ep", label: "Electrum" },
  { key: "sp", label: "Silver" },
  { key: "cp", label: "Copper" },
] as const;
export type CoinKey = (typeof COINS)[number]["key"];
export type Coins = Record<CoinKey, number>;

/** Short answers, at most 100 characters each. */
export const PERSONA_SHORT = [
  { key: "age", label: "Age" },
  { key: "height", label: "Height" },
  { key: "weight", label: "Weight" },
  { key: "eyes", label: "Eyes" },
  { key: "skin", label: "Skin" },
  { key: "hair", label: "Hair" },
  { key: "gender", label: "Gender" },
  { key: "faith", label: "Faith" },
] as const;
/** Paragraphs, at most 8000 characters each. */
export const PERSONA_LONG = [
  { key: "personality_traits", label: "Personality traits" },
  { key: "ideals", label: "Ideals" },
  { key: "bonds", label: "Bonds" },
  { key: "flaws", label: "Flaws" },
  { key: "backstory", label: "Backstory" },
  { key: "allies_and_organizations", label: "Allies & organisations" },
] as const;
export const PERSONA_SHORT_MAX = 100;
export const PERSONA_LONG_MAX = 8000;

function wholeOr(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isInteger(value)
    ? value
    : fallback;
}

/** The classes stored on `trait_data`, skipping any entry it cannot read. */
export function readClasses(traitData: Json): ClassEntry[] {
  const list = Array.isArray(traitData.classes) ? traitData.classes : [];
  return list.flatMap((entry): ClassEntry[] => {
    if (!entry || typeof entry !== "object") return [];
    const row = entry as Json;
    const name = typeof row.name === "string" ? row.name : "";
    const hitDie = HIT_DICE.find((die) => die === row.hit_die);
    if (!name || !hitDie) return [];
    return [
      {
        name,
        subclass: typeof row.subclass === "string" ? row.subclass : "",
        level: Math.min(20, Math.max(1, wholeOr(row.level, 1))),
        hitDie,
      },
    ];
  });
}

/**
 * What a write of the classes carries. With classes stored, `level` is their
 * sum and `class` the first one listed, which is what the validator checks.
 */
export function classesPatch(classes: ClassEntry[]): Json {
  const stored = classes.map((entry) => ({
    name: entry.name.trim(),
    ...(entry.subclass.trim() ? { subclass: entry.subclass.trim() } : {}),
    level: entry.level,
    hit_die: entry.hitDie,
  }));
  if (stored.length === 0) return { classes: [] };
  return {
    classes: stored,
    class: stored[0].name,
    level: stored.reduce((sum, entry) => sum + entry.level, 0),
  };
}

/** Why a set of classes cannot be saved, or null when it can. */
export function classesProblem(classes: ClassEntry[]): string | null {
  if (classes.some((entry) => !entry.name.trim())) {
    return "Every class needs a name.";
  }
  const total = classes.reduce((sum, entry) => sum + entry.level, 0);
  if (total > 20) return `The levels add up to ${total}; the most is 20.`;
  return null;
}

export function readDefences(traitData: Json): Record<DefenceKey, string[]> {
  const read = (key: DefenceKey, choices: readonly string[]) => {
    const list = Array.isArray(traitData[key]) ? traitData[key] : [];
    return choices.filter((id) => list.includes(id));
  };
  return Object.fromEntries(
    DEFENCES.map(({ key, choices }) => [key, read(key, choices)]),
  ) as Record<DefenceKey, string[]>;
}

export function readCoins(resourceData: Json): Coins {
  const stored =
    resourceData.coins && typeof resourceData.coins === "object"
      ? (resourceData.coins as Json)
      : {};
  return Object.fromEntries(
    COINS.map(({ key }) => [key, Math.max(0, wholeOr(stored[key], 0))]),
  ) as Coins;
}

export function readPersona(traitData: Json): Record<string, string> {
  return Object.fromEntries(
    [...PERSONA_SHORT, ...PERSONA_LONG].map(({ key }) => [
      key,
      typeof traitData[key] === "string" ? (traitData[key] as string) : "",
    ]),
  );
}

export function label(id: string): string {
  return id.charAt(0).toUpperCase() + id.slice(1);
}
