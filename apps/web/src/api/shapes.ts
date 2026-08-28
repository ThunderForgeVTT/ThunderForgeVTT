import { postGraphQL } from "@/api/graphqlClient";
import type {
  CreateShapeInput,
  ShapeRecord,
  UpdateShapeInput,
} from "@/types/shape";

const SHAPE_FIELDS = `
  shapeId
  sceneId
  kind
  geometry
  text
  style
  visibleToPlayers
  metadata
  createdBy
  updatedBy
  createdAt
  updatedAt
`;

type ShapesQuery = {
  shapes: ShapeRecord[];
};

type CreateShapeMutation = {
  createShape: ShapeRecord;
};

type UpdateShapeMutation = {
  updateShape: ShapeRecord;
};

type DeleteShapeMutation = {
  deleteShape: boolean;
};

/**
 * Fetch every shape on a scene. Used both for the initial load and as the
 * "refetch on notify" step of real-time shape sync (see
 * engine/world/sync/shapes.ts): the world_events NOTIFY payload only
 * carries the changed shape's id and scene, so on receipt we re-fetch
 * this list rather than trying to reconstruct a shape from the notify
 * payload alone.
 */
export function getShapes(sceneId: string): Promise<ShapeRecord[]> {
  return postGraphQL<ShapesQuery>(
    `
      query SceneShapes($sceneId: UUID!) {
        shapes(sceneId: $sceneId) {
          ${SHAPE_FIELDS}
        }
      }
    `,
    { sceneId },
  ).then((data) => data.shapes);
}

export function createShape(input: CreateShapeInput): Promise<ShapeRecord> {
  return postGraphQL<CreateShapeMutation>(
    `
      mutation CreateShape($input: GraphQLCreateShapeInput!) {
        createShape(input: $input) {
          ${SHAPE_FIELDS}
        }
      }
    `,
    { input },
  ).then((data) => data.createShape);
}

export function updateShape(
  shapeId: string,
  input: UpdateShapeInput,
): Promise<ShapeRecord> {
  return postGraphQL<UpdateShapeMutation>(
    `
      mutation UpdateShape($shapeId: UUID!, $input: GraphQLUpdateShapeInput!) {
        updateShape(shapeId: $shapeId, input: $input) {
          ${SHAPE_FIELDS}
        }
      }
    `,
    { shapeId, input },
  ).then((data) => data.updateShape);
}

export function deleteShape(shapeId: string): Promise<boolean> {
  return postGraphQL<DeleteShapeMutation>(
    `
      mutation DeleteShape($shapeId: UUID!) {
        deleteShape(shapeId: $shapeId)
      }
    `,
    { shapeId },
  ).then((data) => data.deleteShape);
}
