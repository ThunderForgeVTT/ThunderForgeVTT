/**
 * What a card on the Players page says a person *is* and what they *hold*.
 *
 * # Why this is words and not controls
 *
 * The owner, on this page: a Game Master "really [has] control over the
 * entire thing", and the card should say so — "Owner / Game Master", then
 * "playing all characters". The page said neither. It showed a role badge
 * and a picker, and a reader was left to work out from which controls
 * appeared that the person in the first card runs the table. A card that
 * needs its controls read to be understood is not saying anything.
 *
 * # A Game Master's own character is extra, not instead
 *
 * The owner again: a Game Master may set "their own specific character as
 * more of an ephemeral thing... like a genie blessing". So a bound character
 * never replaces "playing all characters" — it is said *beside* it. A
 * sentence that swapped one for the other would tell the table the Game
 * Master had stepped down to one seat.
 *
 * Kept out of the component for the same reason `playerFilter.ts` is: the
 * web suite runs in `node`, and this is the part of the card that can be
 * wrong.
 */
import {
  isWorldMemberRole,
  roleLabel,
  runsTheWorld,
  type WorldMemberRole,
} from "@/types/world";

export interface StandingInput {
  role: string;
  claimedActor: { label: string } | null;
}

export interface PlayerStanding {
  /** "Owner / Game Master", "Game Master", "Player", … */
  title: string;
  /** Whether this person runs the table, and so plays every character. */
  runsTheTable: boolean;
  /** One sentence: what they play. */
  holding: string;
}

/**
 * The title a card leads with.
 *
 * An Owner is always a Game Master too — the server's own gate
 * (`is_dm_of_world`) treats them as one — and the owner of this product asked
 * for both words, so both are said. An unreadable role is shown as the server
 * spelled it rather than guessed at.
 */
export function standingTitle(role: string): string {
  if (!isWorldMemberRole(role)) return role;
  return role === "Owner" ? "Owner / Game Master" : roleLabel(role);
}

export function describeStanding(member: StandingInput): PlayerStanding {
  const role: WorldMemberRole | null = isWorldMemberRole(member.role)
    ? member.role
    : null;
  const runsTheTable = runsTheWorld(role);
  const character = member.claimedActor?.label ?? null;

  let holding: string;
  if (runsTheTable) {
    holding = character
      ? `Playing all characters, with ${character} as their own.`
      : "Playing all characters.";
  } else {
    holding = character ? `Playing ${character}.` : "No character yet.";
  }

  return { title: standingTitle(member.role), runsTheTable, holding };
}
