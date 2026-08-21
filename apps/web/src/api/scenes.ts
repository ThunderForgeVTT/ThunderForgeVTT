import { withCsrf } from "@/api/auth";
import type { SceneRecord } from "@/types/scene";

type GraphQLError = {
  message?: string;
};

type GraphQLResponse<TData> = {
  data?: TData;
  errors?: GraphQLError[];
};

const GRAPHQL_ENDPOINT = "/api/graphql";

const SCENE_FIELDS = `
  sceneId
  worldId
  name
  description
  type
  gridSize
  gridType
  width
  height
  backgroundImagePath
  ownerId
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

type ScenesQuery = {
  scenes: SceneRecord[];
};

/**
 * Fetch every scene in a world, for the scene switcher (WorldPage). Each
 * scene carries its own walls/lights/shapes and optional imported
 * background art (`backgroundImagePath`) — switching scenes means
 * re-pointing all of that at a different `sceneId`, not just changing a
 * label.
 */
export function getScenes(worldId: string): Promise<SceneRecord[]> {
  return postGraphQL<ScenesQuery>(
    `
      query WorldScenes($worldId: UUID!) {
        scenes(worldId: $worldId) {
          ${SCENE_FIELDS}
        }
      }
    `,
    { worldId },
  ).then((data) => data.scenes);
}

export type CreateSceneInput = {
  worldId: string;
  name: string;
  description?: string;
  gridSize?: number;
  gridType?: string;
  width?: number;
  height?: number;
};

type CreateSceneMutation = {
  createScene: SceneRecord;
};

/**
 * Create a new scene in a world (scene owner = the caller, enforced
 * server-side). Used by the SceneSwitcher's "New scene" flow — the
 * primary way a GM gets a second map to switch to.
 */
export function createScene(input: CreateSceneInput): Promise<SceneRecord> {
  return postGraphQL<CreateSceneMutation>(
    `
      mutation CreateScene($input: GraphQLCreateSceneInput!) {
        createScene(input: $input) {
          ${SCENE_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createScene);
}
