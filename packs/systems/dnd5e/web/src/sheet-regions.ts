/**
 * What the D&D 5e character sheet is made of, declared rather than drawn.
 *
 * `ActorSheet.tsx` draws this list. Moving Skills beside the scores, or
 * giving Spellcasting the full width, is an edit to a row here, not to JSX.
 *
 * # Mirrored, not fetched
 *
 * The ability, skill and class tables mirror `packs/systems/dnd5e/system.json`
 * (`abilities`, `skills`) and `packs/systems/dnd5e/server/src/srd.rs`
 * (`CLASSES`). That is this pack's convention for manifest data the browser
 * needs at render time, the same way `derived-data.ts` mirrors `spellSlots`.
 * Keep them in sync: a skill id here is the id the server's rules read from
 * `proficiency_data.skill_proficiencies`, and the id a roll button binds to
 * (`skillSleightOfHand` is `sleight_of_hand` in camel case).
 */

export type AbilityId =
  | "strength"
  | "dexterity"
  | "constitution"
  | "intelligence"
  | "wisdom"
  | "charisma";

export interface AbilityDeclaration {
  id: AbilityId;
  label: string;
  abbreviation: string;
}

export const DND5E_ABILITIES: readonly AbilityDeclaration[] = [
  { id: "strength", label: "Strength", abbreviation: "STR" },
  { id: "dexterity", label: "Dexterity", abbreviation: "DEX" },
  { id: "constitution", label: "Constitution", abbreviation: "CON" },
  { id: "intelligence", label: "Intelligence", abbreviation: "INT" },
  { id: "wisdom", label: "Wisdom", abbreviation: "WIS" },
  { id: "charisma", label: "Charisma", abbreviation: "CHA" },
];

export interface SkillDeclaration {
  id: string;
  label: string;
  ability: AbilityId;
}

/** The eighteen skills, in the order the sheet lists them. */
export const DND5E_SKILLS: readonly SkillDeclaration[] = [
  { id: "acrobatics", label: "Acrobatics", ability: "dexterity" },
  { id: "animal_handling", label: "Animal Handling", ability: "wisdom" },
  { id: "arcana", label: "Arcana", ability: "intelligence" },
  { id: "athletics", label: "Athletics", ability: "strength" },
  { id: "deception", label: "Deception", ability: "charisma" },
  { id: "history", label: "History", ability: "intelligence" },
  { id: "insight", label: "Insight", ability: "wisdom" },
  { id: "intimidation", label: "Intimidation", ability: "charisma" },
  { id: "investigation", label: "Investigation", ability: "intelligence" },
  { id: "medicine", label: "Medicine", ability: "wisdom" },
  { id: "nature", label: "Nature", ability: "intelligence" },
  { id: "perception", label: "Perception", ability: "wisdom" },
  { id: "performance", label: "Performance", ability: "charisma" },
  { id: "persuasion", label: "Persuasion", ability: "charisma" },
  { id: "religion", label: "Religion", ability: "intelligence" },
  { id: "sleight_of_hand", label: "Sleight of Hand", ability: "dexterity" },
  { id: "stealth", label: "Stealth", ability: "dexterity" },
  { id: "survival", label: "Survival", ability: "wisdom" },
];

export interface ClassDeclaration {
  name: string;
  hitDie: number;
  /** Which ability the class casts with; `null` for a class that does not. */
  spellcasting: AbilityId | null;
}

/** The twelve SRD classes. Mirrors `srd.rs`'s `CLASSES` plus the casting stat. */
export const DND5E_CLASSES: readonly ClassDeclaration[] = [
  { name: "Barbarian", hitDie: 12, spellcasting: null },
  { name: "Bard", hitDie: 8, spellcasting: "charisma" },
  { name: "Cleric", hitDie: 8, spellcasting: "wisdom" },
  { name: "Druid", hitDie: 8, spellcasting: "wisdom" },
  { name: "Fighter", hitDie: 10, spellcasting: null },
  { name: "Monk", hitDie: 8, spellcasting: null },
  { name: "Paladin", hitDie: 10, spellcasting: "charisma" },
  { name: "Ranger", hitDie: 10, spellcasting: "wisdom" },
  { name: "Rogue", hitDie: 8, spellcasting: null },
  { name: "Sorcerer", hitDie: 6, spellcasting: "charisma" },
  { name: "Warlock", hitDie: 8, spellcasting: "charisma" },
  { name: "Wizard", hitDie: 6, spellcasting: "intelligence" },
];

export const DND5E_SIZES = [
  "tiny",
  "small",
  "medium",
  "large",
  "huge",
  "gargantuan",
] as const;

export const DND5E_ALIGNMENTS = [
  "Lawful Good",
  "Neutral Good",
  "Chaotic Good",
  "Lawful Neutral",
  "True Neutral",
  "Chaotic Neutral",
  "Lawful Evil",
  "Neutral Evil",
  "Chaotic Evil",
  "Unaligned",
] as const;

/** Experience needed to reach each level, index 1..20. */
export const DND5E_XP_THRESHOLDS: readonly number[] = [
  0, 0, 300, 900, 2700, 6500, 14000, 23000, 34000, 48000, 64000, 85000, 100000,
  120000, 140000, 165000, 195000, 225000, 265000, 305000, 355000,
];

export type SheetRegionKind =
  | "identity"
  | "abilities"
  | "combat"
  | "skills"
  | "spellcasting"
  | "proficiencies"
  | "features"
  | "notes";

export interface SheetRegion {
  id: string;
  kind: SheetRegionKind;
  title: string;
  blurb: string;
  /** Columns the region takes at `lg` (three columns); one column below `md`. */
  span: 1 | 2 | 3;
}

/**
 * The sheet, top to bottom, as it is read at a table: who the character is
 * and what they can do, then how they fare in a fight, then the long lists.
 */
export const DND5E_SHEET_REGIONS: readonly SheetRegion[] = [
  {
    id: "identity",
    kind: "identity",
    title: "Character",
    blurb: "Class, level and origin. Level sets the proficiency bonus.",
    span: 1,
  },
  {
    id: "abilities",
    kind: "abilities",
    title: "Ability scores",
    blurb: "Scores from 1 to 20. Modifiers and saving throws follow.",
    span: 2,
  },
  {
    id: "combat",
    kind: "combat",
    title: "Combat",
    blurb: "Hit points, armour class, initiative, speed and hit dice.",
    span: 1,
  },
  {
    id: "skills",
    kind: "skills",
    title: "Skills",
    blurb:
      "Tick a proficiency and the bonus updates. Roll from the checks below.",
    span: 2,
  },
  {
    id: "spellcasting",
    kind: "spellcasting",
    title: "Spellcasting",
    blurb:
      "Save DC and attack bonus follow the casting ability. Slots per level.",
    span: 2,
  },
  {
    id: "proficiencies",
    kind: "proficiencies",
    title: "Proficiencies & languages",
    blurb: "Armour, weapons, tools and languages, one per line.",
    span: 1,
  },
  {
    id: "features",
    kind: "features",
    title: "Features & traits",
    blurb: "Class features, racial traits and feats, one per line.",
    span: 2,
  },
  {
    id: "notes",
    kind: "notes",
    title: "Notes",
    blurb: "Backstory, allies, bonds. Free text, saved when you say so.",
    span: 1,
  },
];
