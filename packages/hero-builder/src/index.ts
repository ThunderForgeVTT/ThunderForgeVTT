/**
 * @thunderforge/hero-builder — the hero builder, for any React host.
 *
 *   <HeroBuilder initialSpec={{ name: "Sir Pip" }} idPrefix="npc-42" onChange={save} />
 *
 * Every control is generated from `@thunderforge/heroes`, so a part added
 * there appears here with no edit. React comes from the host (a peer
 * dependency); the library has no network, no storage and no routing, and
 * performs no action of its own — saving, uploading and exporting are the
 * host's, through `actions` and the helpers below.
 */
export { HeroBuilder, type HeroBuilderProps } from "./HeroBuilder.tsx";
export { HeroPreview, type HeroPreviewProps } from "./preview/HeroPreview.tsx";
export { renderHero } from "./preview/renderHero.ts";
export type { HeroProblem } from "./problems.ts";
export {
  fileStem,
  heroFiles,
  parseHeroText,
  saveHeroFile,
  type HeroFile,
} from "./io/io.ts";
