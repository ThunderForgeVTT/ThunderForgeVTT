import { postGraphQL } from "@/api/graphqlClient";
import { onViewedLevel, viewedLevelId } from "@/api/viewedLevel";
import type {
  CreateShapeInput,
  ShapeCreator,
  ShapeRecord,
  UpdateShapeInput,
} from "@/types/shape";

const SHAPE_FIELDS = `
  shapeId
  sceneId
  levelId
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
 *
 * Scene levels: answers for one level — the one named, else the one this
 * browser is showing (`viewedLevel.ts`), else whichever the server opens on.
 * A level this viewer may not read answers empty rather than failing.
 */
export function getShapes(
  sceneId: string,
  levelId?: string,
): Promise<ShapeRecord[]> {
  return postGraphQL<ShapesQuery>(
    `
      query SceneShapes($sceneId: UUID!, $levelId: UUID) {
        shapes(sceneId: $sceneId, levelId: $levelId) {
          ${SHAPE_FIELDS}
        }
      }
    `,
    { sceneId, levelId: levelId ?? viewedLevelId(sceneId) },
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
    { input: onViewedLevel(input) },
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

/**
 * Delete a scene's drawings, on every level (spec 082): every one, or only
 * those `createdBy` names. The Game Master's alone. Answers how many went;
 * each also arrives as a `deleted` event, which is what clears the boards.
 */
export function clearShapes(
  sceneId: string,
  createdBy?: string[],
): Promise<number> {
  return postGraphQL<{ clearShapes: number }>(
    `
      mutation ClearShapes($sceneId: UUID!, $createdBy: [UUID!]) {
        clearShapes(sceneId: $sceneId, createdBy: $createdBy)
      }
    `,
    { sceneId, createdBy },
  ).then((data) => data.clearShapes);
}

/**
 * Who drew on a scene, with how many shapes each, leaving out whoever runs
 * the world. The Game Master's alone; for "Clear a player's shapes…".
 */
export function getShapeCreators(sceneId: string): Promise<ShapeCreator[]> {
  return postGraphQL<{ shapeCreators: ShapeCreator[] }>(
    `
      query ShapeCreators($sceneId: UUID!) {
        shapeCreators(sceneId: $sceneId) {
          userId
          displayName
          isMember
          shapeCount
        }
      }
    `,
    { sceneId },
  ).then((data) => data.shapeCreators);
}
