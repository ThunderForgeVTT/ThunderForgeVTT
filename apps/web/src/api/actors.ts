import { withCsrf } from "@/api/auth";
import type { ActorPermissionLevel, ActorPermissionRecord, WorldActorRecord } from "@/types/actor";

type GraphQLError = {
  message?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLError[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";

const WORLD_ACTOR_FIELDS = `
  id
  worldId
  sceneId
  actorType
  gameSystemId
  label
  isPublic
  isNpc
  createdBy
  ownedBy
  myPermissionLevel
  createdAt
  updatedAt
`;

async function postGraphQL<TData>(
  query: string,
  variables?: Record<string, unknown>,
): Promise<TData> {
  const response = await fetch(GRAPHQL_ENDPOINT, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({
      query,
      variables,
    }),
  });

  const payload = (await response.json()) as GraphQLResponse<TData>;
  if (!response.ok) {
    throw new Error(payload.errors?.[0]?.message || "GraphQL request failed");
  }

  if (payload.errors?.length) {
    throw new Error(payload.errors[0]?.message || "GraphQL request failed");
  }

  if (!payload.data) {
    throw new Error("GraphQL response did not include data");
  }

  return payload.data;
}

type WorldActorsQuery = {
  worldActors: WorldActorRecord[];
};

/**
 * Fetch every actor (NPCs and player characters, distinguished by `isNpc`)
 * in a world, across every scene — used by the GM staging page and the
 * full-screen sidebar's NPC/combat roster (spec 009), and by spec 010's
 * actor detail/edit routes.
 */
export function getWorldActors(worldId: string): Promise<WorldActorRecord[]> {
  return postGraphQL<WorldActorsQuery>(
    `
      query WorldActors($worldId: UUID!) {
        worldActors(worldId: $worldId) {
          ${WORLD_ACTOR_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.worldActors);
}

/**
 * Fetch a single actor by id — implemented by filtering a `worldActors`
 * fetch client-side (no dedicated single-actor query exists yet), used by
 * `ActorDetailPage.tsx`'s view/edit routes.
 */
export async function getActor(
  worldId: string,
  actorId: string,
): Promise<WorldActorRecord | null> {
  const actors = await getWorldActors(worldId);
  return actors.find((actor) => actor.id === actorId) ?? null;
}

type CreateActorInput = {
  worldId: string;
  label: string;
  isNpc: boolean;
  actorType?: string;
  gameSystemId?: string;
};

type CreateActorMutation = {
  createActor: WorldActorRecord;
};

/** DM-only (FR-019). Creates a new actor in the world's default scene. */
export function createActor(input: CreateActorInput): Promise<WorldActorRecord> {
  return postGraphQL<CreateActorMutation>(
    `
      mutation CreateActor($input: CreateActorInput!) {
        createActor(input: $input) {
          ${WORLD_ACTOR_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createActor);
}

type UpdateActorInput = {
  actorId: string;
  label?: string;
  isNpc?: boolean;
  actorType?: string;
};

type UpdateActorMutation = {
  updateActor: WorldActorRecord;
};

/** Requires Editor or Owner effective permission (FR-010, FR-011). */
export function updateActor(input: UpdateActorInput): Promise<WorldActorRecord> {
  return postGraphQL<UpdateActorMutation>(
    `
      mutation UpdateActor($input: UpdateActorInput!) {
        updateActor(input: $input) {
          ${WORLD_ACTOR_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.updateActor);
}

type ActorPermissionsQuery = {
  actorPermissions: ActorPermissionRecord[];
};

/** DM-only (FR-014). Returns only explicit ownership-block rows. */
export function getActorPermissions(actorId: string): Promise<ActorPermissionRecord[]> {
  return postGraphQL<ActorPermissionsQuery>(
    `
      query ActorPermissions($actorId: UUID!) {
        actorPermissions(actorId: $actorId) {
          actorId
          userId
          level
          updatedAt
        }
      }
    `,
    { actorId },
  ).then((data) => data.actorPermissions);
}

type SetActorPermissionMutation = {
  setActorPermission: ActorPermissionRecord;
};

/** DM-only (FR-014). UPSERT on (actorId, userId). */
export function setActorPermission(
  actorId: string,
  userId: string,
  level: ActorPermissionLevel,
): Promise<ActorPermissionRecord> {
  return postGraphQL<SetActorPermissionMutation>(
    `
      mutation SetActorPermission($input: SetActorPermissionInput!) {
        setActorPermission(input: $input) {
          actorId
          userId
          level
          updatedAt
        }
      }
    `,
    { input: { actorId, userId, level } },
  ).then((data) => data.setActorPermission);
}

type RemoveActorPermissionMutation = {
  removeActorPermission: boolean;
};

/** DM-only (FR-014). Idempotent — reverts the member to default Viewer. */
export function removeActorPermission(actorId: string, userId: string): Promise<boolean> {
  return postGraphQL<RemoveActorPermissionMutation>(
    `
      mutation RemoveActorPermission($actorId: UUID!, $userId: UUID!) {
        removeActorPermission(actorId: $actorId, userId: $userId)
      }
    `,
    { actorId, userId },
  ).then((data) => data.removeActorPermission);
}
