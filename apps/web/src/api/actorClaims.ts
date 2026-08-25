import { postGraphQL } from "@/api/graphqlClient";
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
export function getMyActorClaim(worldId: string): Promise<ActorClaimRecord | null> {
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
export function getAvailableActors(worldId: string): Promise<WorldActorRecord[]> {
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
export function claimActor(worldId: string, actorId: string): Promise<ActorClaimRecord> {
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
