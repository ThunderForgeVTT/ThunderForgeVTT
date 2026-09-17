/**
 * The smallest spec that draws the same hero (FR-009).
 *
 * A builder holds every field a user touched, including ones set back to
 * what they would have been anyway. Saving that would pin derived colours —
 * a trim that happens to equal the outfit's shade would stop following the
 * outfit. So a field is dropped exactly when dropping it leaves
 * `resolveHero` unchanged, and never by a list of which fields derive.
 */
import { resolveHero, type HeroSpec, type ResolvedHero } from "./spec.ts";

function same(a: ResolvedHero, b: ResolvedHero): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** Throws `HeroSpecError` for a spec that is not a hero. */
export function minimalSpec(spec: HeroSpec): HeroSpec {
  const target = resolveHero(spec);
  const kept: Record<string, unknown> = { ...spec };
  for (const field of Object.keys(spec)) {
    if (field === "name") continue;
    const without = { ...kept };
    delete without[field];
    if (same(resolveHero(without as HeroSpec), target)) {
      delete kept[field];
    }
  }
  return kept as HeroSpec;
}
