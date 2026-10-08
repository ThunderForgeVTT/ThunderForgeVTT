/**
 * walls.ts
 * Wall sync: the wall half of engine/world/sync, mirroring the token
 * event dispatch path (db/collections/worldTokensCollection.ts's
 * setupWorldTokensReplication) and the token store-bridge wiring
 * (bevy/index.ts's bindWorldStore / this directory's index.ts).
 *
 * Two independent responsibilities, per specs/001-bevy-canvas-authoring
 * tasks T007 and T020:
 *
 * 1. Inbound (T007): the server emits a generic `world_events` NOTIFY
 *    (subscription field `worldEventsCreated(worldId)`) with
 *    `eventCode = 10` for any wall create/update/delete
 *    (crates/thunderforge-server/src/world_events.rs::EVENT_CODE_WALL_CHANGED). The
 *    notify payload only carries `{ action, wallId, sceneId }` — not the
 *    full wall — so `applyWallWorldEvent` re-fetches the scene's walls
 *    via GraphQL (api/walls.ts#getWalls) and dispatches `upsert_wall`/
 *    `remove_wall` into the world store, exactly like
 *    setupWorldTokensReplication's event_code switch does for tokens.
 *    `startWallEventSync` wraps this in the same
 *    `for await (const event of subscription)` loop
 *    setupWorldTokensReplication uses, so it can be driven by a GraphQL
 *    subscription transport once one is wired up client-side (none of
 *    the token/actor-data paths in this codebase have live subscription
 *    transport wired yet either — see NOTE below).
 *
 * 2. Outbound (T020): the Bevy engine (or the WallTool UI) dispatches
 *    `create_wall`/`update_wall`/`delete_wall` *intent* commands into the
 *    world store (engine/world/types.ts). `startWallMutationBridge`
 *    subscribes to the store, turns each intent into the matching
 *    `createWall`/`updateWall`/`deleteWall` GraphQL mutation
 *    (api/walls.ts), and on success dispatches the confirmed
 *    `upsert_wall`/`remove_wall` command back with source "sync" so both
 *    the UI and the engine (via bevy/index.ts's generic bridge) observe
 *    the durable result.
 *
 * NOTE: as of this feature, no part of apps/web establishes a live
 * GraphQL subscription transport (no apollo-client/graphql-ws usage
 * exists anywhere in the app yet — confirmed by search).
 * `startWallEventSync` is written to the same
 * "reasonable default" shape as setupWorldTokensReplication so it drops
 * in once that transport exists; until then, wall changes made by the
 * current tab still work end-to-end via the outbound mutation bridge and
 * its optimistic upsert_wall dispatch — only *other* clients' changes
 * won't be observed without a page refresh (which re-fetches via
 * getWalls). This mirrors the exact same limitation tokens already have.
 */

import { setDoorSecret } from "@/api/interactives";
import { createWall, deleteWall, getWalls, updateWall } from "@/api/walls";
import type { WallRecord, DoorState as ApiDoorState } from "@/types/wall";
import type { WorldStore } from "../store";
import type { DoorState, WorldWall } from "../types";
import { localGroupStamp, settleGroup } from "./groupMoves";

type WorldEventLike = {
  event_code?: number;
  eventCode?: number;
  token_event?: unknown;
  tokenEvent?: unknown;
};

const WALL_EVENT_CODE = 10;

function toDoorState(value: ApiDoorState | DoorState | string): DoorState {
  const normalized = value.toString().toLowerCase();
  if (normalized === "open" || normalized === "closed") {
    return normalized;
  }
  return "none";
}

function wallRecordToWorldWall(record: WallRecord): WorldWall {
  return {
    id: record.wallId,
    sceneId: record.sceneId,
    x1: record.x1,
    y1: record.y1,
    x2: record.x2,
    y2: record.y2,
    blocksVision: record.blocksVision,
    blocksMovement: record.blocksMovement,
    doorState: toDoorState(record.doorState),
    locked: record.locked,
    secret: record.secret,
  };
}

function toApiDoorState(value: DoorState): ApiDoorState {
  switch (value) {
    case "open":
      return "OPEN";
    case "closed":
      return "CLOSED";
    default:
      return "NONE";
  }
}

/**
 * Apply a single `world_events` NOTIFY payload to the world store,
 * re-fetching the affected scene's walls when it's a wall event
 * (eventCode 10). No-ops for any other event code.
 */
export async function applyWallWorldEvent(
  worldStore: WorldStore,
  sceneId: string,
  event: WorldEventLike,
): Promise<void> {
  const eventCode = event.event_code ?? event.eventCode;
  if (eventCode !== WALL_EVENT_CODE) {
    return;
  }

  const payload = (event.token_event ?? event.tokenEvent) as
    | {
        action?: string;
        wall_id?: string;
        wallId?: string;
        scene_id?: string;
        sceneId?: string;
      }
    | undefined;

  const eventSceneId = payload?.scene_id ?? payload?.sceneId;
  if (eventSceneId && eventSceneId !== sceneId) {
    return;
  }

  const wallId = payload?.wall_id ?? payload?.wallId;
  const action = payload?.action;

  if (action === "deleted") {
    if (wallId) {
      worldStore.dispatch({ type: "remove_wall", wallId }, "sync");
    }
    return;
  }

  // created/updated: the notify payload doesn't carry the full wall, so
  // re-fetch the scene's walls rather than reconstructing one from it.
  const walls = await getWalls(sceneId);
  for (const wall of walls) {
    worldStore.dispatch(
      { type: "upsert_wall", wall: wallRecordToWorldWall(wall) },
      "sync",
    );
  }
}

/**
 * Drive applyWallWorldEvent from a `worldEventsCreated` GraphQL
 * subscription async iterable, mirroring
 * setupWorldTokensReplication's `for await` loop. Returns a cleanup
 * function that stops consuming the subscription.
 */
export function startWallEventSync(
  worldStore: WorldStore,
  sceneId: string,
  graphqlSubscription: AsyncIterable<WorldEventLike>,
): () => void {
  const abortController = new AbortController();

  (async () => {
    try {
      for await (const event of graphqlSubscription) {
        if (abortController.signal.aborted) break;
        await applyWallWorldEvent(worldStore, sceneId, event);
      }
    } catch (error) {
      console.error("World walls event sync error:", error);
    }
  })();

  return () => {
    abortController.abort();
  };
}

/**
 * Load a scene's current walls and seed the world store with them.
 * Call once when a scene is opened, before/alongside the live event
 * sync above.
 */
export async function loadWallsIntoStore(
  worldStore: WorldStore,
  sceneId: string,
): Promise<void> {
  const walls = await getWalls(sceneId);
  for (const wall of walls) {
    worldStore.dispatch(
      { type: "upsert_wall", wall: wallRecordToWorldWall(wall) },
      "sync",
    );
  }
}

/**
 * Bridge wall *intent* commands (create_wall/update_wall/delete_wall —
 * from the WallTool UI or from the Bevy engine) into GraphQL mutations,
 * dispatching the confirmed upsert_wall/remove_wall back into the store
 * on success. Returns an unsubscribe function.
 */
export function startWallMutationBridge(
  worldStore: WorldStore,
  sceneId: string,
): () => void {
  // One wall's mutations, one at a time, in the order they were asked for —
  // the light bridge's rule (spec 045 FR-061). A hide sent while the wall's
  // move is in flight must not land first and be overwritten by the move's
  // answer.
  const inFlight = new Map<string, Promise<void>>();
  const inTurn = (wallId: string, send: () => Promise<void>) => {
    const next = (inFlight.get(wallId) ?? Promise.resolve()).then(send);
    inFlight.set(wallId, next);
    void next.finally(() => {
      if (inFlight.get(wallId) === next) inFlight.delete(wallId);
    });
  };

  /**
   * Put a wall back as the store held it before a refused change (spec 085
   * FR-014). Dispatched with source "sync", it reaches the engine too, which
   * moved or removed the wall before asking.
   */
  const restore = (before: WorldWall | undefined) => {
    if (before) {
      worldStore.dispatch({ type: "upsert_wall", wall: before }, "sync");
    }
  };

  // The walls as they stood before the event being handled. A hide is
  // applied by the store on dispatch, so by the time this subscriber hears
  // it, the store already holds the hidden version.
  let lastWalls = worldStore.getState().walls;

  const unsubscribe = worldStore.subscribe((event) => {
    const prior = lastWalls;
    lastWalls = event.state.walls;

    // Avoid reacting to our own confirmed dispatches.
    if (event.source === "sync") {
      return;
    }

    const { command } = event;

    if (command.type === "create_wall") {
      const { wall } = command;
      void createWall({
        sceneId,
        x1: wall.x1,
        y1: wall.y1,
        x2: wall.x2,
        y2: wall.y2,
        blocksVision: wall.blocksVision,
        blocksMovement: wall.blocksMovement,
        doorState: toApiDoorState(wall.doorState),
      })
        .then((created) => {
          worldStore.dispatch(
            { type: "upsert_wall", wall: wallRecordToWorldWall(created) },
            "sync",
          );
        })
        .catch((error) => {
          console.error("Failed to create wall:", error);
        });
      return;
    }

    if (command.type === "update_wall") {
      const { wallId, changes, group } = command;
      const before = prior[wallId];
      inTurn(wallId, () =>
        updateWall(wallId, {
          x1: changes.x1,
          y1: changes.y1,
          x2: changes.x2,
          y2: changes.y2,
          blocksVision: changes.blocksVision,
          blocksMovement: changes.blocksMovement,
          doorState:
            changes.doorState !== undefined
              ? toApiDoorState(changes.doorState)
              : undefined,
        })
          .then((updated) => {
            worldStore.dispatch(
              { type: "upsert_wall", wall: wallRecordToWorldWall(updated) },
              "sync",
            );
            settleGroup(group, true, "moved");
          })
          .catch((error) => {
            console.error("Failed to update wall:", error);
            restore(before);
            settleGroup(group, false, "moved");
          }),
      );
      return;
    }

    if (command.type === "delete_wall") {
      const { wallId, group } = command;
      const before = prior[wallId];
      // After the wall's edits, so a late answer to one cannot bring it back.
      inTurn(wallId, () =>
        deleteWall(wallId)
          .then((ok) => {
            if (ok) {
              worldStore.dispatch({ type: "remove_wall", wallId }, "sync");
            } else {
              // The server's "no" to a delete is `false`, not an error.
              restore(before);
            }
            settleGroup(group, ok, "deleted");
          })
          .catch((error) => {
            console.error("Failed to delete wall:", error);
            restore(before);
            settleGroup(group, false, "deleted");
          }),
      );
      return;
    }

    if (command.type === "set_walls_hidden") {
      // Spec 085: any wall, door or not, hidden from the table. One mutation
      // per wall, each judged by the server on its own; the store already
      // shows the result, and a refused wall is put back alone.
      const { wallIds, hidden } = command;
      const stamp = localGroupStamp(wallIds.length);
      for (const wallId of wallIds) {
        const before = prior[wallId];
        inTurn(wallId, () =>
          setDoorSecret(wallId, hidden)
            .catch((error) => {
              console.error("Failed to hide wall:", error);
              return false;
            })
            .then((ok) => {
              if (!ok) {
                restore(before);
              }
              settleGroup(stamp, ok, "hidden");
            }),
        );
      }
    }
  });

  return unsubscribe;
}
