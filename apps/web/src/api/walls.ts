import { withCsrf } from "@/api/auth";
import type { CreateWallInput, UpdateWallInput, WallRecord } from "@/types/wall";

type GraphQLError = {
  message?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLError[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";

const WALL_FIELDS = `
  wallId
  sceneId
  x1
  y1
  x2
  y2
  blocksVision
  blocksMovement
  doorState
  metadata
  createdBy
  updatedBy
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

type WallsQuery = {
  walls: WallRecord[];
};

type CreateWallMutation = {
  createWall: WallRecord;
};

type UpdateWallMutation = {
  updateWall: WallRecord;
};

type DeleteWallMutation = {
  deleteWall: boolean;
};

/**
 * Fetch every wall on a scene. Used both for the initial load and as the
 * "refetch on notify" step of real-time wall sync (see
 * engine/world/sync/walls.ts): the world_events NOTIFY payload only
 * carries the changed wall's id and scene, so on receipt we re-fetch this
 * list rather than trying to reconstruct a wall from the notify payload.
 */
export function getWalls(sceneId: string): Promise<WallRecord[]> {
  return postGraphQL<WallsQuery>(
    `
      query SceneWalls($sceneId: UUID!) {
        walls(sceneId: $sceneId) {
          ${WALL_FIELDS}
        }
      }
    `,
    { sceneId },
  ).then((data) => data.walls);
}

export function createWall(input: CreateWallInput): Promise<WallRecord> {
  return postGraphQL<CreateWallMutation>(
    `
      mutation CreateWall($input: GraphQLCreateWallInput!) {
        createWall(input: $input) {
          ${WALL_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createWall);
}

export function updateWall(
  wallId: string,
  input: UpdateWallInput,
): Promise<WallRecord> {
  return postGraphQL<UpdateWallMutation>(
    `
      mutation UpdateWall($wallId: UUID!, $input: GraphQLUpdateWallInput!) {
        updateWall(wallId: $wallId, input: $input) {
          ${WALL_FIELDS}
        }
      }
    `,
    { wallId, input },
  ).then((data) => data.updateWall);
}

export function deleteWall(wallId: string): Promise<boolean> {
  return postGraphQL<DeleteWallMutation>(
    `
      mutation DeleteWall($wallId: UUID!) {
        deleteWall(wallId: $wallId)
      }
    `,
    { wallId },
  ).then((data) => data.deleteWall);
}
