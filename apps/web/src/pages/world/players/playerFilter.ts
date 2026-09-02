/**
 * Spec 031 (FR-033): which player cards a search box leaves showing.
 *
 * Kept out of the component so it can be tested without a DOM — the web
 * suite runs in `node`, and a filter is the part of a card list that can
 * actually be wrong. The component owns the input; this owns the answer.
 */

export interface SearchablePlayer {
  username: string;
  role: string;
  claimedActor: { label: string } | null;
}

/**
 * Matches a player against the search box.
 *
 * All three fields, not just the name: at a table the question is as often
 * "who is playing Aria" or "who are the GMs" as it is "where is Sam".
 * Case-insensitive and substring-based rather than prefix-based, because a
 * roster is small enough that the looser match never produces a confusing
 * result and does find "Aria Nightbloom" from "night".
 */
export function matchesPlayerQuery(
  player: SearchablePlayer,
  query: string,
): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) {
    return true;
  }

  return [player.username, player.role, player.claimedActor?.label ?? ""].some(
    (field) => field.toLowerCase().includes(needle),
  );
}

/** The subset of `players` a query leaves visible, in the original order. */
export function filterPlayers<T extends SearchablePlayer>(
  players: readonly T[],
  query: string,
): T[] {
  return players.filter((player) => matchesPlayerQuery(player, query));
}
