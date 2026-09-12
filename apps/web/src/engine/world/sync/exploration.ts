/**
 * exploration.ts — a player's map, remembered between sessions.
 *
 * Spec 045 US7. Three things have to agree, and each owns a different part:
 *
 * - the **server** says whether a scene remembers at all, and holds the epoch
 *   that makes a Game Master's reset stick;
 * - the **browser** holds what this player actually explored;
 * - the **engine** decides what has been seen, because it is the only thing
 *   that can (Principle I).
 *
 * This is the seam between them, and it owns no opinion about visibility.
 */
import { postGraphQL } from "@/api/graphqlClient";
import {
  forgetExploration,
  readExploration,
  writeExploration,
} from "@/services/exploredAreas";

/** A Game Master reset the fog (`world_events` code 27). */
export const EXPLORATION_RESET_EVENT_CODE = 27;

export interface SceneExploration {
  enabled: boolean;
  epoch: number;
  /** The number this viewer's stored map must be at or above. */
  mine: number;
}

type Query = { sceneExploration: SceneExploration };

export async function getSceneExploration(
  sceneId: string,
): Promise<SceneExploration> {
  const answer = await postGraphQL<Query>(
    `
      query SceneExploration($sceneId: UUID!) {
        sceneExploration(sceneId: $sceneId) {
          enabled
          epoch
          mine
        }
      }
    `,
    { sceneId },
  );
  return answer.sceneExploration;
}

/**
 * Open a scene: tell the engine whether it remembers, and hand back what this
 * player explored — unless a Game Master has reset it since.
 *
 * Returns the epoch now in force, which is what a later save must be stamped
 * with. A save stamped with the old number would look current to the next
 * session and quietly resurrect a map that was reset.
 */
export async function loadExploration(
  userId: string,
  worldId: string,
  sceneId: string,
): Promise<number> {
  const engine = await import("@/engine/bevy");
  let scene: SceneExploration;
  try {
    scene = await getSceneExploration(sceneId);
  } catch (error) {
    // A scene whose state cannot be read is not a scene with no fog: turning
    // exploration off here would show a player the whole map, which is the
    // one outcome a Game Master who turned it on must never get by accident.
    console.error("Failed to read scene exploration:", error);
    return 0;
  }

  await engine.setExploration(scene.enabled);
  if (!scene.enabled) {
    return scene.mine;
  }

  const stored = await readExploration(userId, worldId, sceneId);
  if (!stored || stored.epoch < scene.mine) {
    // Either nothing kept, or kept under an older epoch — a reset happened
    // while this browser was away. Both mean the same thing to the engine.
    await engine.setExploredCells([]);
    if (stored) {
      await forgetExploration(userId, worldId, sceneId);
    }
    return scene.mine;
  }

  await engine.setExploredCells(stored.cells);
  return scene.mine;
}

/**
 * Save what the engine has accumulated, if it has changed.
 *
 * Called on a timer rather than on every frame: the engine adds cells as a
 * token walks, and writing to storage at that rate would be a write per step
 * for no benefit — a session that ends unsaved loses at most the last few
 * seconds of walking, which the player can re-walk by standing still.
 */
export async function saveExploration(
  userId: string,
  worldId: string,
  sceneId: string,
  epoch: number,
): Promise<boolean> {
  const engine = await import("@/engine/bevy");
  const cells = await engine.exploredCells();
  if (cells.length === 0) {
    return false;
  }
  return writeExploration(userId, worldId, sceneId, cells, epoch);
}

type WorldEventLike = { event_code?: number; eventCode?: number };

/**
 * A Game Master reset the fog while this player is looking at it.
 *
 * The epoch is what makes a reset stick for somebody who was away; this is
 * the fast path for somebody who is here. Re-reads rather than trusting the
 * payload, because a reset aimed at one player and a reset aimed at everyone
 * are the same event on the bus, and only the server can say which applies to
 * the person holding this browser.
 */
export async function applyExplorationWorldEvent(
  userId: string,
  worldId: string,
  sceneId: string,
  event: WorldEventLike,
): Promise<number | null> {
  const code = event.event_code ?? event.eventCode;
  if (code !== EXPLORATION_RESET_EVENT_CODE) {
    return null;
  }
  return loadExploration(userId, worldId, sceneId);
}
