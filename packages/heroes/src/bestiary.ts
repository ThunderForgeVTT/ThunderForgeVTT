/**
 * The first bestiary: what a low-level dungeon is made of, plus the few big
 * things a table asks for by name.
 *
 * Each entry is written the way a statblock reader hands one over — a name
 * and a descriptor line — rather than as a finished spec, so the gallery and
 * the tests exercise exactly the path a book import takes. Tuning a creature
 * means tuning the tables in `families.ts`, where every creature of that kind
 * gets the benefit, not hand-fixing one entry here.
 *
 * The names are folklore's, and the drawings are ours (spec 047 FR-050). No
 * statistic appears here: what a goblin does in a fight belongs to a system
 * pack and its licence, and this file must never become a back door for it.
 */
import type { CreatureSource } from "./monsters.ts";

export interface BestiaryEntry {
  /** Stable id: file names and seed rows are keyed by it. */
  slug: string;
  source: CreatureSource;
}

export const BESTIARY: readonly BestiaryEntry[] = [
  { slug: "goblin", source: { name: "Goblin", descriptor: "Small humanoid" } },
  {
    slug: "hobgoblin",
    source: { name: "Hobgoblin", descriptor: "Medium humanoid" },
  },
  {
    slug: "bugbear",
    source: { name: "Bugbear", descriptor: "Medium humanoid" },
  },
  { slug: "kobold", source: { name: "Kobold", descriptor: "Small humanoid" } },
  { slug: "gnoll", source: { name: "Gnoll", descriptor: "Medium humanoid" } },
  { slug: "orc", source: { name: "Orc", descriptor: "Medium humanoid" } },
  { slug: "ogre", source: { name: "Ogre", descriptor: "Large giant" } },
  { slug: "troll", source: { name: "Troll", descriptor: "Large giant" } },
  {
    slug: "fire-giant",
    source: { name: "Fire Giant", descriptor: "Huge giant" },
  },
  {
    slug: "skeleton",
    source: { name: "Skeleton", descriptor: "Medium undead" },
  },
  { slug: "zombie", source: { name: "Zombie", descriptor: "Medium undead" } },
  { slug: "ghoul", source: { name: "Ghoul", descriptor: "Medium undead" } },
  { slug: "lich", source: { name: "Lich", descriptor: "Medium undead" } },
  {
    slug: "dire-wolf",
    source: { name: "Dire Wolf", descriptor: "Large beast" },
  },
  {
    slug: "brown-bear",
    source: { name: "Brown Bear", descriptor: "Large beast" },
  },
  {
    slug: "giant-spider",
    source: { name: "Giant Spider", descriptor: "Large beast" },
  },
  {
    slug: "owlbear",
    source: { name: "Owlbear", descriptor: "Large monstrosity" },
  },
  {
    slug: "minotaur",
    source: { name: "Minotaur", descriptor: "Large monstrosity" },
  },
  {
    slug: "red-dragon",
    source: { name: "Red Dragon", descriptor: "Gargantuan dragon" },
  },
  {
    slug: "white-dragon-wyrmling",
    source: { name: "White Dragon Wyrmling", descriptor: "Medium dragon" },
  },
  {
    slug: "green-drake",
    source: { name: "Green Drake", descriptor: "Medium dragon" },
  },
  { slug: "imp", source: { name: "Imp", descriptor: "Tiny fiend" } },
  {
    slug: "shadow-demon",
    source: { name: "Shadow Demon", descriptor: "Large fiend" },
  },
  {
    slug: "stone-golem",
    source: { name: "Stone Golem", descriptor: "Large construct" },
  },
  { slug: "treant", source: { name: "Treant", descriptor: "Huge plant" } },
  {
    slug: "fire-elemental",
    source: { name: "Fire Elemental", descriptor: "Large elemental" },
  },
  {
    slug: "gelatinous-cube",
    source: { name: "Gelatinous Cube", descriptor: "Large ooze" },
  },
  {
    slug: "sprite",
    source: { name: "Sprite", descriptor: "Tiny fey" },
  },
  {
    slug: "deva",
    source: { name: "Deva", descriptor: "Medium celestial" },
  },
  {
    slug: "gibbering-horror",
    source: { name: "Gibbering Horror", descriptor: "Large aberration" },
  },
];
