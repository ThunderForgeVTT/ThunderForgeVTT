/**
 * @thunderforge/heroes — little heroes, drawn from parts.
 *
 *   const hero = createHero({ name: "Sir Pip", hair: "short", headgear: "helm" });
 *   hero.portrait(); // square card, SVG
 *   hero.token();    // round token, SVG
 *
 * A hero is a spec: a name and whichever parts and colours differ from the
 * defaults. The parts and their choices are exported as data (`HERO_PARTS`,
 * `HERO_COLORS`, `HERO_FLAGS`) so a hero builder can offer every choice
 * without a list of its own, and `validateHero` checks a spec that arrived as
 * JSON before anything draws it.
 *
 * Monsters are the same factory. `createMonster({ name, descriptor })` takes
 * about as much as a statblock read out of a book gives — "Troll", "Large
 * giant" — and decides the rest from the name, so the same name always draws
 * the same troll:
 *
 *   createMonster({ name: "Red Dragon", descriptor: "Gargantuan dragon" });
 *   monsterPack({ name: "Goblin" }, 6); // six goblins, not one six times
 */
export { HEX_COLOR, INK } from "./color.ts";
export {
  createHero,
  renderPortrait,
  renderToken,
  type Hero,
  type RenderOptions,
} from "./render.ts";
export { PRESET_HEROES, type HeroPreset } from "./presets.ts";
export { BESTIARY, type BestiaryEntry } from "./bestiary.ts";
export {
  CREATURES,
  FAMILIES,
  TINTS,
  type CreatureEntry,
  type CreatureKind,
  type CreatureName,
  type FamilyName,
} from "./families.ts";
export {
  createMonster,
  monsterPack,
  monsterSpec,
  readCreature,
  type CreatureReading,
  type CreatureSource,
} from "./monsters.ts";
export { seeded } from "./seed.ts";
export { HERO_LABELS, labelProblems, type HeroLabels } from "./labels.ts";
export { HERO_PALETTES } from "./palettes.ts";
export {
  HERO_RACES,
  matchRace,
  raceProblems,
  type RaceKey,
  type RaceLook,
} from "./races.ts";
export { randomHero, type RolledHero, type RollOptions } from "./random.ts";
export { minimalSpec } from "./minimal.ts";
export { presetSource } from "./presetSource.ts";
export { HERO_SPEC_SCHEMA, heroSpecSchemaText } from "./schema.ts";
export {
  BUILDS,
  EAR_SHAPES,
  EMBLEMS,
  EYE_STYLES,
  HAIR_STYLES,
  HEADGEAR,
  HERO_COLORS,
  HERO_FLAGS,
  HERO_PARTS,
  HERO_TEXT_LIMITS,
  HeroSpecError,
  HIDES,
  MONSTER_TONES,
  MOUTHS,
  MUZZLES,
  PROPS,
  resolveHero,
  SIZE_CATEGORIES,
  SIZES,
  SKIN_TONES,
  TAILS,
  validateHero,
  WINGS,
  type Build,
  type EarShape,
  type Emblem,
  type EyeStyle,
  type HairStyle,
  type Headgear,
  type HeroSpec,
  type HeroValidation,
  type Hide,
  type Mouth,
  type Muzzle,
  type Prop,
  type ResolvedHero,
  type SizeCategory,
  type Tail,
  type Wings,
} from "./spec.ts";
