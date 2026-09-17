import {
  renderPortrait,
  renderToken,
  type HeroSpec,
} from "@thunderforge/heroes";

/**
 * The two SVG strings for a spec, with every id inside them prefixed by
 * `idPrefix` (FR-011, contract B2): the portrait's ids are
 * `${idPrefix}-portrait-…` and the token's `${idPrefix}-token-…`, so the two
 * never collide with each other, and a host keeps prefixes unique across the
 * document. The host uploads these; the library never does.
 *
 * Throws for a prefix `renderPortrait` refuses (a letter, then letters,
 * digits, `-` or `_`) and for a spec that is not a hero — callers validate
 * first.
 */
export function renderHero(
  spec: HeroSpec,
  idPrefix: string,
): { portrait: string; token: string } {
  return {
    portrait: renderPortrait(spec, { idPrefix }),
    token: renderToken(spec, { idPrefix }),
  };
}
