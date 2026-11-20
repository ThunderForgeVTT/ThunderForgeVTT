import type { WorldMemberRecord } from "@/api/worldMembers";
import type { WorldActorRecord } from "@/types/actor";
import { isWorldMemberRole, runsTheWorld } from "@/types/world";

/** One place at the table: a player, a character, or both. */
export interface Seat {
  key: string;
  /** Who sits here; `null` for a character no one has claimed. */
  player: string | null;
  /** Their character, if they have one. */
  actorId: string | null;
  character: string | null;
}

/**
 * Spec 100 US3.2, "who is coming": every player with the character they
 * claimed, then each character waiting to be claimed as an open seat.
 *
 * Those who run the world are named apart, not seated: they sit behind the
 * screen. A character is an open seat only when it is a player character
 * offered for claiming (spec 017 FR-004); an NPC is never a seat.
 */
export function partySeats(
  members: readonly WorldMemberRecord[],
  actors: readonly WorldActorRecord[],
): { gms: string[]; seats: Seat[] } {
  const runs = (member: WorldMemberRecord) =>
    isWorldMemberRole(member.role) && runsTheWorld(member.role);
  const gms = members.filter(runs).map((member) => member.username);
  const seated = members
    .filter((member) => !runs(member))
    .map((member) => ({
      key: member.id,
      player: member.username,
      actorId: member.claimedActor?.id ?? null,
      character: member.claimedActor?.label ?? null,
    }));
  const claimed = new Set(members.map((member) => member.claimedActor?.id));
  const open = actors
    .filter(
      (actor) =>
        !actor.isNpc &&
        actor.availableForClaim &&
        actor.claimedBy === null &&
        !claimed.has(actor.id),
    )
    .map((actor) => ({
      key: actor.id,
      player: null,
      actorId: actor.id,
      character: actor.label,
    }));
  return { gms, seats: [...seated, ...open] };
}
