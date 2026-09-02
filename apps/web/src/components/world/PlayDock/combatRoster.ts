import type { CombatantRecord } from "@/types/combat";
import type { TokenRecord } from "@/types/token";
import type { WorldActorRecord } from "@/types/actor";

/**
 * Turning a map selection into a combat roster the GM is *offered*.
 *
 * # Why an offer and not an action
 *
 * Spec 031 FR-030 asks that selected tokens be "offerable" as the roster, and
 * the playtest finding behind it is narrow: the GM had already selected
 * exactly the tokens they meant and still had to add them one at a time. The
 * obvious reading — replace the roster with the selection — was rejected. A
 * selection changes on every stray click, and combat rosters accumulate
 * deliberately (a summon added mid-fight, a downed combatant kept in the
 * order). Silently rewriting one from a click would destroy work no undo in
 * this panel could give back. So this module only ever computes *additions*,
 * and a person presses a button to apply them.
 *
 * # Why the roster is matched, not just appended to
 *
 * Selecting the whole party twice must not put everyone in the order twice.
 * Anything the combat already contains is reported separately so the panel can
 * say so rather than offering a duplicate that the GM would then have to
 * remove.
 *
 * # Why a selected id can resolve to nothing
 *
 * The engine spawns demo tokens that were never persisted, and a token placed
 * a moment ago may not be in the fetched scene tokens yet. Those ids are
 * reported rather than dropped, so the caller can go and look again once
 * instead of offering a blank row or refetching forever.
 */

/** One selected token, resolved into what a combatant needs. */
export interface RosterCandidate {
  tokenId: string;
  /** The actor this token stands for, when it stands for one. */
  actorId: string | null;
  label: string;
  isNpc: boolean;
}

export interface RosterOffer {
  /** Candidates the combat does not contain yet, in selection order. */
  additions: RosterCandidate[];
  /** Candidates the combat already contains. */
  alreadyPresent: RosterCandidate[];
  /** Selected ids no persisted scene token answers to. */
  unresolvedTokenIds: string[];
}

/** What a token with neither an actor nor a written label is called. */
const UNNAMED = "Unnamed token";

function tokenLabel(
  token: TokenRecord,
  actor: WorldActorRecord | undefined,
): string {
  if (actor) return actor.label;
  const written = token.metadata?.label;
  // A blank metadata label is the same as none: the token sync writes the key
  // through from whatever authored it, so an empty string reaches here.
  if (typeof written === "string" && written.trim() !== "") {
    return written.trim();
  }
  return UNNAMED;
}

/**
 * Whether `combatant` is already the entry for `candidate`.
 *
 * Token identity wins where both carry one, because two tokens of the same
 * actor (a duplicated minion, a character and their mirror image) are two
 * combatants and must each get a turn. Actor identity is the fallback so a
 * combatant added through the actor picker is still recognised as the same
 * participant when their token is later selected.
 */
function matches(
  combatant: CombatantRecord,
  candidate: RosterCandidate,
): boolean {
  if (combatant.tokenId !== null)
    return combatant.tokenId === candidate.tokenId;
  return candidate.actorId !== null && combatant.actorId === candidate.actorId;
}

export function buildRosterOffer(input: {
  selectedTokenIds: string[];
  tokens: TokenRecord[];
  actors: WorldActorRecord[];
  combatants: CombatantRecord[];
}): RosterOffer {
  const tokensById = new Map(
    input.tokens.map((token) => [token.tokenId, token]),
  );
  const actorsById = new Map(input.actors.map((actor) => [actor.id, actor]));

  const additions: RosterCandidate[] = [];
  const alreadyPresent: RosterCandidate[] = [];
  const unresolvedTokenIds: string[] = [];
  const seen = new Set<string>();

  for (const tokenId of input.selectedTokenIds) {
    // The engine reports a stack topmost-first and a caller may concatenate
    // selections; the same token twice is still one combatant.
    if (seen.has(tokenId)) continue;
    seen.add(tokenId);

    const token = tokensById.get(tokenId);
    if (!token) {
      unresolvedTokenIds.push(tokenId);
      continue;
    }

    const actor = token.actorId ? actorsById.get(token.actorId) : undefined;
    const candidate: RosterCandidate = {
      tokenId,
      // An actor id the world does not list is carried anyway: the server owns
      // that reference, and dropping it here would file a token-only combatant
      // for a participant that does have a sheet.
      actorId: token.actorId,
      label: tokenLabel(token, actor),
      isNpc: actor ? actor.isNpc : token.tokenType === "npc",
    };

    if (input.combatants.some((combatant) => matches(combatant, candidate))) {
      alreadyPresent.push(candidate);
    } else {
      additions.push(candidate);
    }
  }

  return { additions, alreadyPresent, unresolvedTokenIds };
}

/**
 * Ids worth asking the server about, given what has already been asked.
 *
 * Guards the refetch loop: a demo token never becomes a persisted one, so
 * "some selected id is unresolved" would otherwise mean "fetch the scene's
 * tokens forever". One attempt per id is enough to catch the case this exists
 * for — a token placed seconds ago — and costs nothing for the case it does
 * not.
 */
export function unattemptedIds(
  unresolvedTokenIds: string[],
  attempted: ReadonlySet<string>,
): string[] {
  return unresolvedTokenIds.filter((id) => !attempted.has(id));
}

/** Whether two selections are the same, in the same order. */
export function sameTokenIds(
  a: readonly string[],
  b: readonly string[],
): boolean {
  return a.length === b.length && a.every((id, index) => id === b[index]);
}
