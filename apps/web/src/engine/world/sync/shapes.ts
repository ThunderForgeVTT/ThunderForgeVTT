/**
 * shapes.ts
 * Shape sync: the shape half of engine/world/sync, mirroring walls.ts's
 * inbound NOTIFY-refetch + outbound mutation-bridge design (which itself
 * mirrors the token event dispatch path). Per
 * specs/001-bevy-canvas-authoring tasks T058:
 *
 * 1. Inbound: the server emits a generic `world_events` NOTIFY
 *    (subscription field `worldEventsCreated(worldId)`) with
 *    `eventCode = 12` for any shape create/update/delete
 *    (src/server/src/world_events.rs::EVENT_CODE_SHAPE_CHANGED). The
 *    notify payload only carries `{ action, shapeId, sceneId }` — not the
 *    full shape — so `applyShapeWorldEvent` re-fetches the scene's shapes
 *    via GraphQL (api/shapes.ts#getShapes) and dispatches
 *    `upsert_shape`/`remove_shape` into the world store, exactly like
 *    applyWallWorldEvent's event_code switch does for walls.
 *    `startShapeEventSync` wraps this in the same
 *    `for await (const event of subscription)` loop walls.ts uses, so it
 *    can be driven by a GraphQL subscription transport once one is wired
 *    up client-side.
 *
 * 2. Outbound: the Bevy engine (or the ShapeTool UI) dispatches
 *    `create_shape`/`update_shape`/`delete_shape` *intent* commands into
 *    the world store (engine/world/types.ts). `startShapeMutationBridge`
 *    subscribes to the store, turns each intent into the matching
 *    `createShape`/`updateShape`/`deleteShape` GraphQL mutation
 *    (api/shapes.ts), and on success dispatches the confirmed
 *    `upsert_shape`/`remove_shape` command back with source "sync" so
 *    both the UI and the engine (via bevy/index.ts's generic bridge)
 *    observe the durable result.
 *
 * NOTE (same caveat as walls.ts): no part of apps/web establishes a live
 * GraphQL subscription transport yet. `startShapeEventSync` is written
 * to the same "reasonable default" shape as walls.ts's
 * `startWallEventSync` so it drops in once that transport exists; until
 * then, shape changes made by the current tab still work end-to-end via
 * the outbound mutation bridge and its optimistic upsert_shape dispatch
 * — only *other* clients' changes won't be observed without a page
 * refresh (which re-fetches via getShapes).
 *
 * Coordination note: the engine-side (Rust/WASM) half of the
 * create_shape/update_shape/delete_shape <-> upsert_shape/remove_shape
 * contract is being built in parallel; this module is written to the
 * JSON shape documented in the T058 task description so it should drop
 * in once that lands. It compiles/typechecks standalone in the
 * meantime.
 */

import { createShape, deleteShape, getShapes, updateShape } from "@/api/shapes";
import type { ShapeRecord, ShapeKind as ApiShapeKind } from "@/types/shape";
import type { WorldStore } from "../store";
import type { ShapeKind, WorldShape } from "../types";

type WorldEventLike = {
  event_code?: number;
  eventCode?: number;
  token_event?: unknown;
  tokenEvent?: unknown;
};

const SHAPE_EVENT_CODE = 12;

function toShapeKind(value: ApiShapeKind | ShapeKind | string): ShapeKind {
  const normalized = value.toString().toLowerCase();
  if (
    normalized === "rect" ||
    normalized === "ellipse" ||
    normalized === "line" ||
    normalized === "text"
  ) {
    return normalized;
  }
  return "stroke";
}

function shapeRecordToWorldShape(record: ShapeRecord): WorldShape {
  return {
    id: record.shapeId,
    sceneId: record.sceneId,
    kind: toShapeKind(record.kind),
    geometry: record.geometry,
    text: record.text,
    style: record.style,
    visibleToPlayers: record.visibleToPlayers,
  };
}

function toApiShapeKind(value: ShapeKind): ApiShapeKind {
  switch (value) {
    case "rect":
      return "RECT";
    case "ellipse":
      return "ELLIPSE";
    case "line":
      return "LINE";
    case "text":
      return "TEXT";
    default:
      return "STROKE";
  }
}

/**
 * Apply a single `world_events` NOTIFY payload to the world store,
 * re-fetching the affected scene's shapes when it's a shape event
 * (eventCode 12). No-ops for any other event code.
 */
export async function applyShapeWorldEvent(
  worldStore: WorldStore,
  sceneId: string,
  event: WorldEventLike,
): Promise<void> {
  const eventCode = event.event_code ?? event.eventCode;
  if (eventCode !== SHAPE_EVENT_CODE) {
    return;
  }

  const payload = (event.token_event ?? event.tokenEvent) as
    | {
        action?: string;
        shape_id?: string;
        shapeId?: string;
        scene_id?: string;
        sceneId?: string;
      }
    | undefined;

  const eventSceneId = payload?.scene_id ?? payload?.sceneId;
  if (eventSceneId && eventSceneId !== sceneId) {
    return;
  }

  const shapeId = payload?.shape_id ?? payload?.shapeId;
  const action = payload?.action;

  if (action === "deleted") {
    if (shapeId) {
      worldStore.dispatch({ type: "remove_shape", shapeId }, "sync");
    }
    return;
  }

  // created/updated: the notify payload doesn't carry the full shape, so
  // re-fetch the scene's shapes rather than reconstructing one from it.
  const shapes = await getShapes(sceneId);
  for (const shape of shapes) {
    worldStore.dispatch(
      { type: "upsert_shape", shape: shapeRecordToWorldShape(shape) },
      "sync",
    );
  }
}

/**
 * Drive applyShapeWorldEvent from a `worldEventsCreated` GraphQL
 * subscription async iterable, mirroring startWallEventSync's `for
 * await` loop. Returns a cleanup function that stops consuming the
 * subscription.
 */
export function startShapeEventSync(
  worldStore: WorldStore,
  sceneId: string,
  graphqlSubscription: AsyncIterable<WorldEventLike>,
): () => void {
  const abortController = new AbortController();

  (async () => {
    try {
      for await (const event of graphqlSubscription) {
        if (abortController.signal.aborted) break;
        await applyShapeWorldEvent(worldStore, sceneId, event);
      }
    } catch (error) {
      console.error("World shapes event sync error:", error);
    }
  })();

  return () => {
    abortController.abort();
  };
}

/**
 * Load a scene's current shapes and seed the world store with them.
 * Call once when a scene is opened, before/alongside the live event
 * sync above.
 */
export async function loadShapesIntoStore(
  worldStore: WorldStore,
  sceneId: string,
): Promise<void> {
  const shapes = await getShapes(sceneId);
  for (const shape of shapes) {
    worldStore.dispatch(
      { type: "upsert_shape", shape: shapeRecordToWorldShape(shape) },
      "sync",
    );
  }
}

/**
 * Bridge shape *intent* commands (create_shape/update_shape/delete_shape
 * — from the ShapeTool UI or from the Bevy engine) into GraphQL
 * mutations, dispatching the confirmed upsert_shape/remove_shape back
 * into the store on success. Returns an unsubscribe function.
 */
export function startShapeMutationBridge(
  worldStore: WorldStore,
  sceneId: string,
): () => void {
  const unsubscribe = worldStore.subscribe((event) => {
    // Avoid reacting to our own confirmed dispatches.
    if (event.source === "sync") {
      return;
    }

    const { command } = event;

    if (command.type === "create_shape") {
      const { shape } = command;
      void createShape({
        sceneId,
        kind: toApiShapeKind(shape.kind),
        geometry: shape.geometry,
        text: shape.text ?? undefined,
        style: shape.style ?? undefined,
        visibleToPlayers: shape.visibleToPlayers,
      })
        .then((created) => {
          worldStore.dispatch(
            { type: "upsert_shape", shape: shapeRecordToWorldShape(created) },
            "sync",
          );
        })
        .catch((error) => {
          console.error("Failed to create shape:", error);
        });
      return;
    }

    if (command.type === "update_shape") {
      const { shapeId, changes } = command;
      void updateShape(shapeId, {
        geometry: changes.geometry,
        text: changes.text ?? undefined,
        style: changes.style ?? undefined,
        visibleToPlayers: changes.visibleToPlayers,
      })
        .then((updated) => {
          worldStore.dispatch(
            { type: "upsert_shape", shape: shapeRecordToWorldShape(updated) },
            "sync",
          );
        })
        .catch((error) => {
          console.error("Failed to update shape:", error);
        });
      return;
    }

    if (command.type === "delete_shape") {
      const { shapeId } = command;
      void deleteShape(shapeId)
        .then((ok) => {
          if (ok) {
            worldStore.dispatch({ type: "remove_shape", shapeId }, "sync");
          }
        })
        .catch((error) => {
          console.error("Failed to delete shape:", error);
        });
    }
  });

  return unsubscribe;
}
