import type { WorldActorRecord } from "@/types/actor";

/**
 * Whether the person looking at this actor may change it.
 *
 * One function because the question is asked in more than one place — the
 * full actor page, and the character sheet a player opens in the play dock —
 * and the two must never disagree: a player who can edit their character on
 * its page and not in the dock (or the reverse) is looking at a bug, whichever
 * of the two is "right".
 *
 * The answer is the server's. `myPermissionLevel` is resolved per caller when
 * the actor is read (Game Master, owner, holder of a claim, explicit share),
 * and every mutation behind an editable sheet is checked again there
 * (Principle III), so this only decides what is *offered*. Anything above
 * Viewer may edit; there is deliberately no second rule here to drift from
 * that one.
 */
export function mayEditActor(
  actor: Pick<WorldActorRecord, "myPermissionLevel">,
): boolean {
  return actor.myPermissionLevel !== "VIEWER";
}
