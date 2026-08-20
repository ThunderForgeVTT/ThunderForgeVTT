/**
 * lights.ts
 * Light sync: the light-source half of engine/world/sync, mirroring
 * walls.ts's inbound/outbound shape exactly (which itself mirrors the
 * token event dispatch path in db/collections/worldTokensCollection.ts's
 * setupWorldTokensReplication and the token store-bridge wiring in
 * bevy/index.ts's bindWorldStore / this directory's index.ts).
 *
 * Two independent responsibilities, per specs/001-bevy-canvas-authoring
 * task T043:
 *
 * 1. Inbound: the server emits a generic `world_events` NOTIFY
 *    (subscription field `worldEventsCreated(worldId)`) with
 *    `eventCode = 11` for any light source create/update/delete
 *    (src/server/src/world_events.rs::EVENT_CODE_LIGHT_SOURCE_CHANGED).
 *    The notify payload only carries `{ action, lightId, sceneId }` — not
 *    the full light — so `applyLightWorldEvent` re-fetches the scene's
 *    lights via GraphQL (api/lights.ts#getLights) and dispatches
 *    `upsert_light`/`remove_light` into the world store, exactly like
 *    applyWallWorldEvent does for walls.
 *
 * 2. Outbound: the Bevy engine (or the LightingTool UI) dispatches
 *    `create_light`/`update_light`/`delete_light` *intent* commands into
 *    the world store (engine/world/types.ts). `startLightMutationBridge`
 *    subscribes to the store, turns each intent into the matching
 *    `createLightSource`/`updateLightSource`/`deleteLightSource` GraphQL
 *    mutation (api/lights.ts), and on success dispatches the confirmed
 *    `upsert_light`/`remove_light` command back with source "sync" so
 *    both the UI and the engine (via bevy/index.ts's generic bridge)
 *    observe the durable result.
 *
 * NOTE: as of this feature, no part of apps/web establishes a live
 * GraphQL subscription transport (see walls.ts's identical note).
 * `startLightEventSync` is written to the same "reasonable default" shape
 * as startWallEventSync so it drops in once that transport exists; until
 * then, light changes made by the current tab still work end-to-end via
 * the outbound mutation bridge and its optimistic upsert_light dispatch —
 * only *other* clients' changes won't be observed without a page refresh
 * (which re-fetches via getLights). This mirrors the exact same
 * limitation walls (and tokens) already have.
 */

import { createLight, deleteLight, getLights, updateLight } from "@/api/lights";
import type { LightRecord } from "@/types/light";
import type { WorldStore } from "../store";
import type { WorldLight } from "../types";
import { getWorldDatabase } from "./database";

async function persistLightDoc(light: LightRecord): Promise<void> {
  try {
    const db = await getWorldDatabase();
    await db.collections.world_lights.upsert({
      lightId: light.lightId,
      sceneId: light.sceneId,
      x: light.x,
      y: light.y,
      radius: light.radius,
      intensity: light.intensity,
      color: light.color,
      attachedTokenId: light.attachedTokenId,
      castsShadows: light.castsShadows,
      metadata: light.metadata,
      createdBy: light.createdBy,
      updatedBy: light.updatedBy,
      createdAt: light.createdAt,
      updatedAt: light.updatedAt,
    });
  } catch (error) {
    // RxDB persistence is a best-effort offline cache (T042); the world
    // store dispatch above is the source of truth for the live session.
    console.error("Failed to persist light source to RxDB:", error);
  }
}

async function removeLightDoc(lightId: string): Promise<void> {
  try {
    const db = await getWorldDatabase();
    const doc = await db.collections.world_lights.findOne(lightId).exec();
    await doc?.remove();
  } catch (error) {
    console.error("Failed to remove light source from RxDB:", error);
  }
}

type WorldEventLike = {
  event_code?: number;
  eventCode?: number;
  token_event?: unknown;
  tokenEvent?: unknown;
};

const LIGHT_EVENT_CODE = 11;

function lightRecordToWorldLight(record: LightRecord): WorldLight {
  return {
    id: record.lightId,
    sceneId: record.sceneId,
    x: record.x,
    y: record.y,
    radius: record.radius,
    intensity: record.intensity,
    color: record.color,
    attachedTokenId: record.attachedTokenId,
    castsShadows: record.castsShadows,
  };
}

/**
 * Apply a single `world_events` NOTIFY payload to the world store,
 * re-fetching the affected scene's lights when it's a light event
 * (eventCode 11). No-ops for any other event code.
 */
export async function applyLightWorldEvent(
  worldStore: WorldStore,
  sceneId: string,
  event: WorldEventLike,
): Promise<void> {
  const eventCode = event.event_code ?? event.eventCode;
  if (eventCode !== LIGHT_EVENT_CODE) {
    return;
  }

  const payload = (event.token_event ?? event.tokenEvent) as
    | { action?: string; light_id?: string; lightId?: string; scene_id?: string; sceneId?: string }
    | undefined;

  const eventSceneId = payload?.scene_id ?? payload?.sceneId;
  if (eventSceneId && eventSceneId !== sceneId) {
    return;
  }

  const lightId = payload?.light_id ?? payload?.lightId;
  const action = payload?.action;

  if (action === "deleted") {
    if (lightId) {
      worldStore.dispatch({ type: "remove_light", lightId }, "sync");
      await removeLightDoc(lightId);
    }
    return;
  }

  // created/updated: the notify payload doesn't carry the full light
  // source, so re-fetch the scene's lights rather than reconstructing one
  // from it.
  const lights = await getLights(sceneId);
  for (const light of lights) {
    worldStore.dispatch(
      { type: "upsert_light", light: lightRecordToWorldLight(light) },
      "sync",
    );
    await persistLightDoc(light);
  }
}

/**
 * Drive applyLightWorldEvent from a `worldEventsCreated` GraphQL
 * subscription async iterable, mirroring startWallEventSync's `for await`
 * loop. Returns a cleanup function that stops consuming the subscription.
 */
export function startLightEventSync(
  worldStore: WorldStore,
  sceneId: string,
  graphqlSubscription: AsyncIterable<WorldEventLike>,
): () => void {
  const abortController = new AbortController();

  (async () => {
    try {
      for await (const event of graphqlSubscription) {
        if (abortController.signal.aborted) break;
        await applyLightWorldEvent(worldStore, sceneId, event);
      }
    } catch (error) {
      console.error("World lights event sync error:", error);
    }
  })();

  return () => {
    abortController.abort();
  };
}

/**
 * Load a scene's current light sources and seed the world store with
 * them. Call once when a scene is opened, before/alongside the live
 * event sync above.
 */
export async function loadLightsIntoStore(
  worldStore: WorldStore,
  sceneId: string,
): Promise<void> {
  const lights = await getLights(sceneId);
  for (const light of lights) {
    worldStore.dispatch(
      { type: "upsert_light", light: lightRecordToWorldLight(light) },
      "sync",
    );
    await persistLightDoc(light);
  }
}

/**
 * Bridge light *intent* commands (create_light/update_light/delete_light
 * — from the LightingTool UI or from the Bevy engine) into GraphQL
 * mutations, dispatching the confirmed upsert_light/remove_light back
 * into the store on success. Returns an unsubscribe function.
 */
export function startLightMutationBridge(
  worldStore: WorldStore,
  sceneId: string,
): () => void {
  const unsubscribe = worldStore.subscribe((event) => {
    // Avoid reacting to our own confirmed dispatches.
    if (event.source === "sync") {
      return;
    }

    const { command } = event;

    if (command.type === "create_light") {
      const { light } = command;
      void createLight({
        sceneId,
        x: light.x,
        y: light.y,
        radius: light.radius,
        intensity: light.intensity,
        color: light.color,
        attachedTokenId: light.attachedTokenId,
        castsShadows: light.castsShadows,
      })
        .then((created) => {
          worldStore.dispatch(
            { type: "upsert_light", light: lightRecordToWorldLight(created) },
            "sync",
          );
          void persistLightDoc(created);
        })
        .catch((error) => {
          console.error("Failed to create light source:", error);
        });
      return;
    }

    if (command.type === "update_light") {
      const { lightId, changes } = command;
      void updateLight(lightId, {
        x: changes.x,
        y: changes.y,
        radius: changes.radius,
        intensity: changes.intensity,
        color: changes.color,
        attachedTokenId: changes.attachedTokenId,
        castsShadows: changes.castsShadows,
      })
        .then((updated) => {
          worldStore.dispatch(
            { type: "upsert_light", light: lightRecordToWorldLight(updated) },
            "sync",
          );
          void persistLightDoc(updated);
        })
        .catch((error) => {
          console.error("Failed to update light source:", error);
        });
      return;
    }

    if (command.type === "delete_light") {
      const { lightId } = command;
      void deleteLight(lightId)
        .then((ok) => {
          if (ok) {
            worldStore.dispatch({ type: "remove_light", lightId }, "sync");
            void removeLightDoc(lightId);
          }
        })
        .catch((error) => {
          console.error("Failed to delete light source:", error);
        });
    }
  });

  return unsubscribe;
}
