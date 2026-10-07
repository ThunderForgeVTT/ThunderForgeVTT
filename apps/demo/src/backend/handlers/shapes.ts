/**
 * A scene's drawings, as the server keeps them since spec 082 (research R10).
 *
 * - `createShape`, `updateShape` and `deleteShape` mirror
 *   `crates/thunderforge-server/src/graphql/mutations_shapes.rs`: the shape
 *   is the tab's viewer's, a player's is always shown to players, and a player
 *   edits or deletes only their own (`auth::shape_authority`).
 * - `clearShapes` mirrors `mutations_clear_shapes.rs`: the Game Master's
 *   alone, every shape on the scene or only some creators', one `deleted`
 *   event per shape.
 * - `shapeCreators` mirrors `queries/shape_creators.rs`: who drew, other than
 *   whoever runs the world.
 *
 * Every change is announced as code 12.
 */
import { DEMO_PLAYER, DEMO_USER } from "../../seed/world";
import { viewerIsGm, viewerUser } from "../actors";
import { EVENT, now, record } from "../events";
import { demoState, markChanged, type DemoState, type Row } from "../state";
import { given, levelFor, onLevel, refuse, type Handler } from "./common";

/** A shape saved before spec 082 has no creator; it was the GM's. */
function creatorOf(shape: Row): string {
  return (shape.createdBy as string | undefined) ?? DEMO_USER.id;
}

function withCreator(shape: Row): Row {
  return {
    ...shape,
    createdBy: creatorOf(shape),
    updatedBy: shape.updatedBy ?? creatorOf(shape),
  };
}

/** The Game Master may touch any shape; a player only their own. */
function mayWrite(state: DemoState, shape: Row): boolean {
  return viewerIsGm(state) || creatorOf(shape) === viewerUser(state).id;
}

function announce(action: string, shape: Row): void {
  record(EVENT.shape, {
    action,
    shape_id: shape.shapeId,
    scene_id: shape.sceneId,
    level_id: shape.levelId,
  });
}

export const shapeQueries: Record<string, Handler> = {
  // `scene.rs`: a player sees only the shapes shown to players.
  shapes: (args) => {
    const state = demoState();
    return onLevel(state.shapes, args)
      .filter((shape) => viewerIsGm(state) || shape.visibleToPlayers === true)
      .map(withCreator);
  },
  shapeCreators: ({ sceneId }) => {
    const state = demoState();
    if (!viewerIsGm(state)) refuse("Scene not found");
    // The demo's one player is the only creator who is not the GM.
    const count = state.shapes.filter(
      (shape) =>
        shape.sceneId === sceneId && creatorOf(shape) === DEMO_PLAYER.id,
    ).length;
    if (count === 0) return [];
    return [
      {
        userId: DEMO_PLAYER.id,
        displayName: DEMO_PLAYER.username,
        isMember: true,
        shapeCount: count,
      },
    ];
  },
};

export const shapeMutations: Record<string, Handler> = {
  createShape: ({ input }) => {
    const state = demoState();
    const at = now();
    const viewer = viewerUser(state).id;
    const shape: Row = {
      shapeId: crypto.randomUUID(),
      sceneId: input.sceneId,
      levelId: levelFor(state, input.sceneId, input.levelId),
      kind: input.kind,
      geometry: input.geometry,
      text: input.text ?? null,
      style: input.style ?? null,
      visibleToPlayers: viewerIsGm(state)
        ? (input.visibleToPlayers ?? true)
        : true,
      metadata: input.metadata ?? null,
      createdBy: viewer,
      updatedBy: viewer,
      createdAt: at,
      updatedAt: at,
    };
    state.shapes.push(shape);
    announce("created", shape);
    markChanged();
    return shape;
  },
  updateShape: ({ shapeId, input }) => {
    const state = demoState();
    const shape = state.shapes.find((row) => row.shapeId === shapeId);
    if (!shape || !mayWrite(state, shape)) {
      refuse("Failed to update shape (not found or not owned by you)");
    }
    const changes = given(input);
    // A player cannot hide their own shape.
    if (!viewerIsGm(state)) delete changes.visibleToPlayers;
    Object.assign(shape, changes, {
      createdBy: creatorOf(shape),
      updatedBy: viewerUser(state).id,
      updatedAt: now(),
    });
    announce("updated", shape);
    markChanged();
    return shape;
  },
  // Not allowed reads as "nothing deleted", as the server answers it.
  deleteShape: ({ shapeId }) => {
    const state = demoState();
    const index = state.shapes.findIndex((row) => row.shapeId === shapeId);
    if (index < 0 || !mayWrite(state, state.shapes[index])) return false;
    const [shape] = state.shapes.splice(index, 1);
    announce("deleted", shape);
    markChanged();
    return true;
  },
  clearShapes: ({ sceneId, createdBy }) => {
    const state = demoState();
    if (!viewerIsGm(state)) refuse("Failed to clear shapes (scene not found)");
    const creators = createdBy as string[] | null | undefined;
    const cleared = (shape: Row) =>
      shape.sceneId === sceneId &&
      (creators == null || creators.includes(creatorOf(shape)));
    const gone = state.shapes.filter(cleared);
    state.shapes = state.shapes.filter((shape) => !cleared(shape));
    for (const shape of gone) announce("deleted", shape);
    if (gone.length > 0) markChanged();
    return gone.length;
  },
};
