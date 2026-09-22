import { validateHero, type HeroSpec } from "@thunderforge/heroes";
import {
  ACTOR_IMAGE_PORTRAIT,
  ACTOR_IMAGE_TOKEN,
  type ActorImageSpec,
} from "@/api/actors";

/**
 * Spec 044 FR-036, FR-037: which hero the builder opens on for an actor that
 * may already have a look.
 *
 * The portrait's spec is preferred, then the token's, then the actor's name,
 * as in phase (b). Where the two roles disagree — different specs, or only
 * one of them built — the builder says so rather than quietly picking one.
 *
 * A stored spec is untrusted: the catalogue may have lost a choice it names
 * since it was saved. It is checked here with `validateHero` before anything
 * draws it, and a spec that fails is reported by field, never redrawn from
 * defaults (US6 scenario 4).
 */
export type LookSource = "portrait" | "token" | "name";

export type OpeningLook =
  | { source: LookSource; valid: true; spec: HeroSpec; note: string | null }
  | {
      source: Exclude<LookSource, "name">;
      valid: false;
      /** The stored spec as it was stored, for the builder to report on. */
      spec: unknown;
      problems: string[];
      note: string | null;
    };

/** A spec read back from JSONB has its keys in the database's order, so two
 *  specs are compared field by field, not as text. */
function sameSpec(a: unknown, b: unknown): boolean {
  const canonical = (value: unknown) =>
    typeof value === "object" && value !== null && !Array.isArray(value)
      ? JSON.stringify(
          Object.entries(value as Record<string, unknown>).sort(([x], [y]) =>
            x < y ? -1 : x > y ? 1 : 0,
          ),
        )
      : JSON.stringify(value);
  return canonical(a) === canonical(b);
}

const ROLE_NAME = {
  [ACTOR_IMAGE_PORTRAIT]: "portrait",
  [ACTOR_IMAGE_TOKEN]: "token",
} as const;

export function openingLook(
  images: readonly ActorImageSpec[],
  fromName: HeroSpec,
): OpeningLook {
  const row = (role: string) => images.find((image) => image.role === role);
  const portrait = row(ACTOR_IMAGE_PORTRAIT);
  const token = row(ACTOR_IMAGE_TOKEN);
  const portraitSpec = portrait?.heroSpec ?? null;
  const tokenSpec = token?.heroSpec ?? null;

  if (portraitSpec === null && tokenSpec === null) {
    return { source: "name", valid: true, spec: fromName, note: null };
  }

  const fromPortrait = portraitSpec !== null;
  const spec = fromPortrait ? portraitSpec : tokenSpec;
  const other = fromPortrait ? token : portrait;
  const otherSpec = fromPortrait ? tokenSpec : portraitSpec;
  const opened =
    ROLE_NAME[fromPortrait ? ACTOR_IMAGE_PORTRAIT : ACTOR_IMAGE_TOKEN];
  const otherName =
    ROLE_NAME[fromPortrait ? ACTOR_IMAGE_TOKEN : ACTOR_IMAGE_PORTRAIT];

  let note: string | null = null;
  if (otherSpec !== null) {
    if (!sameSpec(spec, otherSpec)) {
      note = `The portrait and the token were built as different heroes. This opens on the ${opened}'s.`;
    }
  } else if (other) {
    note = `The ${otherName} is no longer a built hero: it was replaced by an uploaded image. This opens on the ${opened}'s.`;
  } else {
    note = `Only the ${opened} was built; there is no ${otherName}.`;
  }

  const checked = validateHero(spec);
  if (!checked.ok) {
    return {
      source: opened,
      valid: false,
      spec,
      problems: checked.problems,
      note,
    };
  }
  return { source: opened, valid: true, spec: spec as HeroSpec, note };
}
