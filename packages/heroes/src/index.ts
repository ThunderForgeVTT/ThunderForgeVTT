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
 */
export { HEX_COLOR } from "./color.ts";
export {
  createHero,
  renderPortrait,
  renderToken,
  type Hero,
  type RenderOptions,
} from "./render.ts";
export { PRESET_HEROES, type HeroPreset } from "./presets.ts";
export {
  EAR_SHAPES,
  EMBLEMS,
  HAIR_STYLES,
  HEADGEAR,
  HERO_COLORS,
  HERO_FLAGS,
  HERO_PARTS,
  HeroSpecError,
  MOUTHS,
  PROPS,
  resolveHero,
  SKIN_TONES,
  validateHero,
  type EarShape,
  type Emblem,
  type HairStyle,
  type Headgear,
  type HeroSpec,
  type HeroValidation,
  type Mouth,
  type Prop,
  type ResolvedHero,
} from "./spec.ts";
