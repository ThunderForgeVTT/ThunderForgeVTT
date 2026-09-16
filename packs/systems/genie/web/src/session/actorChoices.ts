/**
 * Who a Game Master may point a session-resource control at.
 *
 * # Why this file exists
 *
 * The Grant Session Resource control and the attributed Puzzle Clock
 * advance both ask the same question — "which actor?" — and both used to
 * answer it with the party: `myActor` plus `partyMembers`, which
 * `useGenieSession` had already filtered down to `!isNpc`. That is the
 * wrong list. A Genie session hands Insight to a shopkeeper as readily as
 * to a player's character, and a world staged with NPCs and no claimed
 * player character offered a dropdown with nothing in it at all (owner,
 * 2026-09-15: "I should be able to select an NPC or character in this
 * dropdown, but really it's about actors").
 *
 * So the list is **actors**, and this module says in one place what order
 * they come in and how they are grouped. It is deliberately pure — the
 * pack's tests run under `node --test`, with no DOM — so the ordering rule
 * is testable without rendering a select.
 *
 * # The order
 *
 * Characters first, NPCs after (the owner's words again: "NPCs should be
 * last where players should be first in the session resources"). Inside a
 * group, by name — but numerically aware, so a roster of `Guard 2`,
 * `Guard 10` reads in the order a Game Master wrote it rather than the
 * order ASCII would put it in. An empty group is not rendered at all; a
 * `<optgroup label="NPCs">` with nothing under it is a lie about the
 * world.
 */

/** The little of an actor a picker needs. Structurally satisfied by
 * `WorldActorRecord`, without this module depending on the host's type. */
export interface ActorChoice {
  id: string;
  label: string;
  isNpc: boolean;
}

export interface ActorChoiceGroup {
  /** The `<optgroup>` label — "Characters" or "NPCs". */
  label: string;
  actors: ActorChoice[];
}

export const CHARACTERS_GROUP_LABEL = "Characters";
export const NPCS_GROUP_LABEL = "NPCs";

/** Name order, with runs of digits compared as numbers (`Guard 2` before
 * `Guard 10`) and case ignored, falling back to id so the order is total
 * and two identically-named NPCs never swap places between renders. */
function byName(a: ActorChoice, b: ActorChoice): number {
  const byLabel = a.label.localeCompare(b.label, undefined, {
    numeric: true,
    sensitivity: "base",
  });
  return byLabel !== 0 ? byLabel : a.id.localeCompare(b.id);
}

/**
 * Group a world's actors for a picker: characters first, NPCs after, each
 * group in name order, empty groups omitted.
 *
 * Whatever reaches here is already what the viewer may see — the server
 * withholds a hidden NPC from a player at the data boundary
 * (`src/server/src/auth/npc_visibility.rs`), so this must not filter
 * again. A Game Master sees their hidden NPCs here precisely because the
 * server sent them.
 */
export function groupActorChoices<T extends ActorChoice>(
  actors: readonly T[],
): ActorChoiceGroup[] {
  const characters = actors.filter((actor) => !actor.isNpc).sort(byName);
  const npcs = actors.filter((actor) => actor.isNpc).sort(byName);

  const groups: ActorChoiceGroup[] = [];
  if (characters.length > 0) {
    groups.push({ label: CHARACTERS_GROUP_LABEL, actors: characters });
  }
  if (npcs.length > 0) {
    groups.push({ label: NPCS_GROUP_LABEL, actors: npcs });
  }
  return groups;
}
