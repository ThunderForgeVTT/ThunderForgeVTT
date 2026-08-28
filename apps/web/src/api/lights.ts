import { postGraphQL } from "@/api/graphqlClient";
import type {
  CreateLightInput,
  LightRecord,
  UpdateLightInput,
} from "@/types/light";

const LIGHT_FIELDS = `
  lightId
  sceneId
  x
  y
  radius
  intensity
  color
  attachedTokenId
  castsShadows
  metadata
  createdBy
  updatedBy
  createdAt
  updatedAt
`;

type LightsQuery = {
  lightSources: LightRecord[];
};

type CreateLightMutation = {
  createLightSource: LightRecord;
};

type UpdateLightMutation = {
  updateLightSource: LightRecord;
};

type DeleteLightMutation = {
  deleteLightSource: boolean;
};

/**
 * Fetch every light source on a scene. Used both for the initial load and
 * as the "refetch on notify" step of real-time light sync (see
 * engine/world/sync/lights.ts): the world_events NOTIFY payload only
 * carries the changed light's id and scene, so on receipt we re-fetch this
 * list rather than trying to reconstruct a light from the notify payload.
 */
export function getLights(sceneId: string): Promise<LightRecord[]> {
  return postGraphQL<LightsQuery>(
    `
      query SceneLights($sceneId: UUID!) {
        lightSources(sceneId: $sceneId) {
          ${LIGHT_FIELDS}
        }
      }
    `,
    { sceneId },
  ).then((data) => data.lightSources);
}

export function createLight(input: CreateLightInput): Promise<LightRecord> {
  return postGraphQL<CreateLightMutation>(
    `
      mutation CreateLightSource($input: GraphQLCreateLightSourceInput!) {
        createLightSource(input: $input) {
          ${LIGHT_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createLightSource);
}

export function updateLight(
  lightId: string,
  input: UpdateLightInput,
): Promise<LightRecord> {
  return postGraphQL<UpdateLightMutation>(
    `
      mutation UpdateLightSource($lightId: UUID!, $input: GraphQLUpdateLightSourceInput!) {
        updateLightSource(lightId: $lightId, input: $input) {
          ${LIGHT_FIELDS}
        }
      }
    `,
    { lightId, input },
  ).then((data) => data.updateLightSource);
}

export function deleteLight(lightId: string): Promise<boolean> {
  return postGraphQL<DeleteLightMutation>(
    `
      mutation DeleteLightSource($lightId: UUID!) {
        deleteLightSource(lightId: $lightId)
      }
    `,
    { lightId },
  ).then((data) => data.deleteLightSource);
}
