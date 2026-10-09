import { mayEditActor } from "@/pages/world/actor/actorEditRight";
import type { WorldActorRecord } from "@/types/actor";

export interface HeroControls {
  /** The editable character sheet (the actor's edit page). */
  sheet: boolean;
  /** The hero builder, which saves a portrait and a token. */
  builder: boolean;
}

/**
 * What a Players-screen card offers for its claimed hero, from the server's
 * answers for the viewer, with no rule of its own.
 *
 * The sheet follows `mayEditActor`, as the actor page and the play dock do.
 * The builder follows `myMayChangeImagery`, as the actor page's "Build look"
 * does, which also folds in the world's player-art setting. An actor this
 * viewer was not sent (or has not been fetched yet) offers nothing.
 */
export function heroControlsFor(
  actor:
    | Pick<WorldActorRecord, "myPermissionLevel" | "myMayChangeImagery">
    | undefined,
): HeroControls {
  if (!actor) {
    return { sheet: false, builder: false };
  }
  return {
    sheet: mayEditActor(actor),
    builder: actor.myMayChangeImagery,
  };
}
