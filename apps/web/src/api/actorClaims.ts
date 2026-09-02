import { GraphQLRequestError, postGraphQL } from "@/api/graphqlClient";
import type { ActorClaimRecord, WorldActorRecord } from "@/types/actor";

const WORLD_ACTOR_FIELDS = `
  id
  worldId
  sceneId
  actorType
  gameSystemId
  label
  description
  isPublic
  isNpc
  createdBy
  ownedBy
  myPermissionLevel
  createdAt
  updatedAt
  loreLinkedFrom {
    id
    title
    slug
  }
  availableForClaim
  claimedBy {
    id
    worldId
    userId
    username
  }
`;

const ACTOR_CLAIM_FIELDS = `
  actorId
  worldMemberId
  claimedByUserId
  claimedAt
  actor {
    ${WORLD_ACTOR_FIELDS}
  }
`;

type MyActorClaimQuery = {
  myActorClaim: ActorClaimRecord | null;
};

/**
 * Spec 017 (FR-001/FR-002/FR-003): `null` for the GM/Owner role (never
 * gated) or a non-GM member with no claim yet; otherwise the claimed
 * character. Drives the Actor Selection routing gate.
 */
export function getMyActorClaim(
  worldId: string,
): Promise<ActorClaimRecord | null> {
  return postGraphQL<MyActorClaimQuery>(
    `
      query MyActorClaim($worldId: UUID!) {
        myActorClaim(worldId: $worldId) {
          ${ACTOR_CLAIM_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.myActorClaim);
}

type AvailableActorsQuery = {
  availableActors: WorldActorRecord[];
};

/** Spec 017 (FR-005): PC-classified, flagged, and currently unclaimed. */
export function getAvailableActors(
  worldId: string,
): Promise<WorldActorRecord[]> {
  return postGraphQL<AvailableActorsQuery>(
    `
      query AvailableActors($worldId: UUID!) {
        availableActors(worldId: $worldId) {
          ${WORLD_ACTOR_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.availableActors);
}

type ClaimActorMutation = {
  claimActor: ActorClaimRecord;
};

/** Spec 017 (FR-006): atomic — a lost race surfaces a specific error. */
export function claimActor(
  worldId: string,
  actorId: string,
): Promise<ActorClaimRecord> {
  return postGraphQL<ClaimActorMutation>(
    `
      mutation ClaimActor($worldId: UUID!, $actorId: UUID!) {
        claimActor(worldId: $worldId, actorId: $actorId) {
          ${ACTOR_CLAIM_FIELDS}
        }
      }
    `,
    { worldId, actorId },
  ).then((data) => data.claimActor);
}

type CreateAndClaimActorMutation = {
  createAndClaimActor: ActorClaimRecord;
};

/**
 * Spec 017 (FR-008/FR-009): rejected server-side if the world's
 * `allowPlayerCreatedActors` setting is off, regardless of client UI state.
 */
export function createAndClaimActor(
  worldId: string,
  name: string,
  description?: string,
): Promise<ActorClaimRecord> {
  return postGraphQL<CreateAndClaimActorMutation>(
    `
      mutation CreateAndClaimActor($worldId: UUID!, $name: String!, $description: String) {
        createAndClaimActor(worldId: $worldId, name: $name, description: $description) {
          ${ACTOR_CLAIM_FIELDS}
        }
      }
    `,
    { worldId, name, description },
  ).then((data) => data.createAndClaimActor);
}

/**
 * The extension code the server sets when a character is already played.
 *
 * Spec 031 FR-034. Three surfaces write this one relation — this module's
 * `claimActor` and `setPlayerCharacterBinding`, and the actor page's release
 * — and all three lose the same race in the same way. Keyed on the code
 * rather than the message, which is written for a person and will change.
 */
export const ALREADY_CLAIMED = "ALREADY_CLAIMED";

/**
 * The extension code the server sets when a release names a claim that has
 * since moved to somebody else.
 *
 * The screen that offered the release was reading a state that no longer
 * exists; the honest response is to re-read it, not to retry the write.
 */
export const CLAIM_CHANGED = "CLAIM_CHANGED";

/** Whether a failed claim or binding was somebody else being quicker. */
export function isAlreadyClaimed(error: unknown): boolean {
  return error instanceof GraphQLRequestError && error.hasCode(ALREADY_CLAIMED);
}

/** Whether a failed release was aimed at a claim that had already moved. */
export function isClaimChanged(error: unknown): boolean {
  return error instanceof GraphQLRequestError && error.hasCode(CLAIM_CHANGED);
}

type SetPlayerCharacterBindingMutation = {
  setPlayerCharacterBinding: WorldActorRecord | null;
};

/**
 * Spec 031 (FR-034): a GM sets which character a player is playing, from
 * the players section. `null` for `actorId` clears the binding.
 *
 * GM authority over the world is checked server-side, and the write goes
 * through the same arbiter as a player's own `claimActor` — so a picker
 * that shows a character as free cannot produce a second claim on it
 * (Constitution Principle III).
 */
export function setPlayerCharacterBinding(
  worldId: string,
  worldMemberId: string,
  actorId: string | null,
): Promise<WorldActorRecord | null> {
  return postGraphQL<SetPlayerCharacterBindingMutation>(
    `
      mutation SetPlayerCharacterBinding($worldId: UUID!, $worldMemberId: UUID!, $actorId: UUID) {
        setPlayerCharacterBinding(worldId: $worldId, worldMemberId: $worldMemberId, actorId: $actorId) {
          ${WORLD_ACTOR_FIELDS}
        }
      }
    `,
    { worldId, worldMemberId, actorId },
  ).then((data) => data.setPlayerCharacterBinding);
}
