import { renderHero } from "@thunderforge/hero-builder";
import type { HeroSpec } from "@thunderforge/heroes";
import {
  ACTOR_IMAGE_PORTRAIT,
  ACTOR_IMAGE_TOKEN,
  uploadActorImage,
  type ActorImageRecord,
} from "@/api/actors";
import { GraphQLRequestError } from "@/api/graphqlClient";
import { WORLD_PLAY_PAUSED } from "@/api/playPauseSignal";
import { heroSvgFile } from "@/pages/world/actor/heroFiles";

/**
 * Spec 044 FR-024, FR-025: storing a built hero on an actor.
 *
 * Every host — the imagery panel, a compendium row, Quick NPC — saves through
 * here, so the rules are written once:
 *
 * - Two ordinary uploads, portrait then token, not a transaction (research
 *   R3). Each replaces its own role and nothing else.
 * - A role is `saved` only when the mutation returned its row (B4). Anything
 *   else is `failed`, with the server's own message, and can be retried alone.
 * - A paused world refuses every upload. The first refusal stands for both
 *   roles — the second would only be refused too — and the transport is told
 *   not to send the page to the pause notice, because the builder is holding
 *   a hero that was not saved and must keep it (spec Edge Cases).
 *
 * Imported only by the builder's dialogs, which the hosts load lazily, so the
 * drawing code never reaches a page that does not open a builder (FR-022).
 */

export const BUILT_ROLES = [ACTOR_IMAGE_PORTRAIT, ACTOR_IMAGE_TOKEN] as const;
export type BuiltRole = (typeof BUILT_ROLES)[number];

export type RoleOutcome =
  | { status: "saved"; image: ActorImageRecord }
  | { status: "failed"; message: string; paused: boolean };

export interface BuiltHeroSave {
  portrait: RoleOutcome;
  token: RoleOutcome;
  /** The world is paused: nothing was stored. */
  paused: boolean;
  /** Upload one role again, alone. */
  retry(role: BuiltRole): Promise<RoleOutcome>;
}

/** Id prefix for the stored drawings. They are rasterised on the server, so
 *  it only has to be a valid prefix, not unique in any document. */
const STORED_PREFIX = "tfh-stored";

function failure(err: unknown): RoleOutcome {
  const paused =
    err instanceof GraphQLRequestError && err.codes.includes(WORLD_PLAY_PAUSED);
  return {
    status: "failed",
    message: err instanceof Error ? err.message : "The upload was refused.",
    paused,
  };
}

async function uploadRole(
  actorId: string,
  role: BuiltRole,
  svg: string,
): Promise<RoleOutcome> {
  try {
    const image = await uploadActorImage(
      actorId,
      role,
      heroSvgFile(svg, role),
      {
        announcePause: false,
      },
    );
    return { status: "saved", image };
  } catch (err) {
    return failure(err);
  }
}

export async function saveBuiltHero(
  actorId: string,
  spec: HeroSpec,
): Promise<BuiltHeroSave> {
  const svgs = renderHero(spec, STORED_PREFIX);
  const retry = (role: BuiltRole) => uploadRole(actorId, role, svgs[role]);

  const portrait = await retry(ACTOR_IMAGE_PORTRAIT);
  if (portrait.status === "failed" && portrait.paused) {
    return { portrait, token: portrait, paused: true, retry };
  }
  const token = await retry(ACTOR_IMAGE_TOKEN);
  const paused = token.status === "failed" && token.paused;
  return { portrait, token, paused, retry };
}

/** The rows a save stored, for a host to show. */
export function savedImages(
  ...outcomes: readonly RoleOutcome[]
): ActorImageRecord[] {
  return outcomes.flatMap((outcome) =>
    outcome.status === "saved" ? [outcome.image] : [],
  );
}
