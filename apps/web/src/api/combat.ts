// Play-view Combat (src/server/src/graphql/mutations_combat.rs).
//
// Every mutation returns the whole combat, already in turn order — this
// client never sorts combatants itself. Turn order is defined once,
// server-side (`sort_combatants`), precisely so the GM and every player
// walk the same sequence.

import { postGraphQL } from "@/api/graphqlClient";
import type { CombatRecord } from "@/types/combat";

const COMBAT_FIELDS = `
  id
  worldId
  sceneId
  round
  activeCombatantId
  endedAt
  combatants {
    id
    combatId
    actorId
    tokenId
    label
    initiative
    tiebreak
    isNpc
    active
  }
`;

/** The world's running combat, or null when none is in progress. */
export function getActiveCombat(worldId: string): Promise<CombatRecord | null> {
  return postGraphQL<{ activeCombat: CombatRecord | null }>(
    `
      query ActiveCombat($worldId: UUID!) {
        activeCombat(worldId: $worldId) {
          ${COMBAT_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.activeCombat);
}

/** GM-only. Idempotent — returns the running combat if one already exists. */
export function startCombat(worldId: string, sceneId?: string | null): Promise<CombatRecord> {
  return postGraphQL<{ startCombat: CombatRecord }>(
    `
      mutation StartCombat($input: StartCombatInput!) {
        startCombat(input: $input) {
          ${COMBAT_FIELDS}
        }
      }
    `,
    { input: { worldId, sceneId } },
  ).then((data) => data.startCombat);
}

export function addCombatant(input: {
  combatId: string;
  label: string;
  actorId?: string | null;
  tokenId?: string | null;
  initiative?: number;
  tiebreak?: number;
  isNpc?: boolean;
}): Promise<CombatRecord> {
  return postGraphQL<{ addCombatant: CombatRecord }>(
    `
      mutation AddCombatant($input: AddCombatantInput!) {
        addCombatant(input: $input) {
          ${COMBAT_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.addCombatant);
}

export function updateCombatant(input: {
  combatantId: string;
  initiative?: number;
  tiebreak?: number;
  active?: boolean;
  label?: string;
}): Promise<CombatRecord> {
  return postGraphQL<{ updateCombatant: CombatRecord }>(
    `
      mutation UpdateCombatant($input: UpdateCombatantInput!) {
        updateCombatant(input: $input) {
          ${COMBAT_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.updateCombatant);
}

export function removeCombatant(combatantId: string): Promise<CombatRecord> {
  return postGraphQL<{ removeCombatant: CombatRecord }>(
    `
      mutation RemoveCombatant($combatantId: UUID!) {
        removeCombatant(combatantId: $combatantId) {
          ${COMBAT_FIELDS}
        }
      }
    `,
    { combatantId },
  ).then((data) => data.removeCombatant);
}

export function advanceTurn(combatId: string): Promise<CombatRecord> {
  return postGraphQL<{ advanceTurn: CombatRecord }>(
    `
      mutation AdvanceTurn($combatId: UUID!) {
        advanceTurn(combatId: $combatId) {
          ${COMBAT_FIELDS}
        }
      }
    `,
    { combatId },
  ).then((data) => data.advanceTurn);
}

export function endCombat(combatId: string): Promise<CombatRecord> {
  return postGraphQL<{ endCombat: CombatRecord }>(
    `
      mutation EndCombat($combatId: UUID!) {
        endCombat(combatId: $combatId) {
          ${COMBAT_FIELDS}
        }
      }
    `,
    { combatId },
  ).then((data) => data.endCombat);
}
