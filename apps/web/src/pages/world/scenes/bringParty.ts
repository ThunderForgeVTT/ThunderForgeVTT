import { postGraphQL } from "@/api/graphqlClient";

/**
 * Spec 031 (T056, FR-019): the Launch surface's "bring the party" call, and
 * the sentence it produces afterwards.
 *
 * # Why this sits beside the page rather than in `api/`
 *
 * Only the scene-change surface performs it. Bringing the party is not a
 * general capability a screen might reach for — it is the second half of one
 * Launch, and ADR-056 is explicit that it is a *choice* attached to that
 * action rather than something the world does on its own.
 *
 * # Why the message is a separate function
 *
 * The web suite runs in `node` with no DOM, so a component's rendered text is
 * not testable here but a pure function's answer is — and what the Game
 * Master is told is the part that can actually be wrong. "Brought 3
 * characters" when two of them were already downstairs is a lie the GM would
 * only catch by counting tokens.
 */

export interface PartyArrival {
  /** Characters that gained a token in the destination. */
  arrivedActorIds: string[];
  /** Characters that already had one, left exactly as they were. */
  alreadyPresentActorIds: string[];
}

type BringPartyMutation = {
  bringPartyToScene: PartyArrival;
};

/**
 * Give every party character a token in `sceneId`, skipping any who already
 * have one.
 *
 * Character ids come back, not tokens: ADR-056 re-creates tokens on arrival,
 * so a token id does not survive a scene change and nothing may hold one
 * across it. The server decides who is in the party when no ids are given.
 */
export function bringPartyToScene(sceneId: string): Promise<PartyArrival> {
  return postGraphQL<BringPartyMutation>(
    `
      mutation BringPartyToScene($input: BringPartyToSceneInput!) {
        bringPartyToScene(input: $input) {
          arrivedActorIds
          alreadyPresentActorIds
        }
      }
    `,
    { input: { sceneId } },
  ).then((data) => data.bringPartyToScene);
}

function characters(count: number): string {
  return count === 1 ? "1 character" : `${count} characters`;
}

/**
 * What to tell the Game Master about an arrival.
 *
 * The already-present half is reported rather than swallowed, because it is
 * the answer to the question a GM asks when the number looks wrong: the party
 * was not partly lost, it was partly already there. Saying nothing would make
 * a correct result look like a bug on every return to a scene.
 */
export function describeArrival(arrival: PartyArrival): string {
  const arrived = arrival.arrivedActorIds.length;
  const present = arrival.alreadyPresentActorIds.length;

  if (arrived === 0 && present === 0) {
    return "No party characters to bring.";
  }
  if (arrived === 0) {
    return `The party was already here — ${characters(present)} unchanged.`;
  }
  if (present === 0) {
    return `Brought ${characters(arrived)}.`;
  }
  return `Brought ${characters(arrived)}; ${present} already here.`;
}
