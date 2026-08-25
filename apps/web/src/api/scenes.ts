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
  summaryMarkdown
  summaryRenderedHtml
  hidden
  previewUrl
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

type SceneQuery = {
  scene: SceneRecord | null;
};

/**
 * Spec 022 (FR-001a): fetch one scene by id — the Scene detail gateway's
 * data source. Returns `null` for a stale/missing/hidden-from-this-caller
 * scene id (server-side filtering mirrors `getScenes`, see queries/scene.rs).
 */
export function getScene(sceneId: string): Promise<SceneRecord | null> {
  return postGraphQL<SceneQuery>(
    `
      query WorldScene($sceneId: UUID!) {
        scene(sceneId: $sceneId) {
          ${SCENE_FIELDS}
        }
      }
    `,
    { sceneId },
  ).then((data) => data.scene);
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

export type UpdateSceneInput = {
  name?: string;
  description?: string;
  gridSize?: number;
  gridType?: string;
  width?: number;
  height?: number;
  /** Spec 022 (FR-005/FR-006): GM-authored Markdown summary source. */
  summaryMarkdown?: string;
};

type UpdateSceneMutation = {
  updateScene: SceneRecord;
};

/** Spec 022: also used by the Scenes section's summary editor
 * (`summaryMarkdown` re-renders `summaryRenderedHtml` server-side). */
export function updateScene(sceneId: string, input: UpdateSceneInput): Promise<SceneRecord> {
  return postGraphQL<UpdateSceneMutation>(
    `
      mutation UpdateScene($sceneId: UUID!, $input: GraphQLUpdateSceneInput!) {
        updateScene(sceneId: $sceneId, input: $input) {
          ${SCENE_FIELDS}
        }
      }
    `,
    { sceneId, input },
  ).then((data) => data.updateScene);
}

type UpdateSceneHiddenMutation = {
  updateSceneHidden: SceneRecord;
};

/** Spec 022 (FR-007): GM/Owner-only, toggles a scene's player-facing visibility. */
export function updateSceneHidden(sceneId: string, hidden: boolean): Promise<SceneRecord> {
  return postGraphQL<UpdateSceneHiddenMutation>(
    `
      mutation UpdateSceneHidden($sceneId: UUID!, $hidden: Boolean!) {
        updateSceneHidden(sceneId: $sceneId, hidden: $hidden) {
          ${SCENE_FIELDS}
        }
      }
    `,
    { sceneId, hidden },
  ).then((data) => data.updateSceneHidden);
}

type LaunchSceneResult = {
  id: string;
  activeSceneId: string | null;
};

type LaunchSceneMutation = {
  launchScene: LaunchSceneResult;
};

/**
 * Spec 022 (FR-002a/FR-002b, ADR-046): GM/Owner-only. Sets the world's
 * server-authoritative active scene and broadcasts the switch to every
 * world member currently in Play — the Scenes section's "Launch" action.
 */
export function launchScene(worldId: string, sceneId: string): Promise<LaunchSceneResult> {
  return postGraphQL<LaunchSceneMutation>(
    `
      mutation LaunchScene($worldId: UUID!, $sceneId: UUID!) {
        launchScene(worldId: $worldId, sceneId: $sceneId) {
          id
          activeSceneId
        }
      }
    `,
    { worldId, sceneId },
  ).then((data) => data.launchScene);
}
