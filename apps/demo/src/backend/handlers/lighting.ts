/**
 * A scene's light and its memory of what players explored, as the server
 * keeps them.
 *
 * - `updateSceneAmbientLight` mirrors `update_scene_ambient_light_impl` in
 *   `crates/thunderforge-server/src/graphql/mutations_scenes.rs`: bright, dim
 *   or dark, the Game Master's alone, announced as code 25. The scene's entry
 *   level carries the same light, as the database trigger keeps it there.
 * - `sceneExploration`, `setSceneExploration` and `resetSceneExploration`
 *   mirror `crates/thunderforge-server/src/exploration.rs` and
 *   `graphql/exploration.rs` (spec 045 US7): the server holds whether a scene
 *   remembers and the epoch that makes a reset stick; the browser holds the
 *   map itself. A reset is announced as code 27.
 */
import { viewerIsGm, viewerUser } from "../actors";
import { EVENT, now, record } from "../events";
import {
  demoState,
  markChanged,
  type DemoState,
  type SceneExploration,
} from "../state";
import { ambientOf, refuse, sceneOf, type Handler } from "./common";

function explorationOf(state: DemoState, sceneId: string): SceneExploration {
  state.exploration ??= {};
  return (state.exploration[sceneId] ??= {
    enabled: false,
    epoch: 0,
    resets: {},
  });
}

function gmOnlyExploration(state: DemoState, sceneId: string): void {
  sceneOf(state, sceneId);
  if (!viewerIsGm(state)) {
    refuse("Only the DM (Owner or GM) may change or reset exploration");
  }
}

export const lightingQueries: Record<string, Handler> = {
  sceneExploration: ({ sceneId }) => {
    const state = demoState();
    sceneOf(state, sceneId);
    const scene = explorationOf(state, sceneId);
    const own = scene.resets[viewerUser(state).id] ?? scene.epoch;
    return {
      enabled: scene.enabled,
      epoch: scene.epoch,
      mine: Math.max(own, scene.epoch),
    };
  },
};

export const lightingMutations: Record<string, Handler> = {
  updateSceneAmbientLight: ({ sceneId, ambientLight }) => {
    const state = demoState();
    const level = ambientOf(ambientLight);
    if (!level) refuse("A scene's light is bright, dim or dark");
    const scene = sceneOf(state, sceneId);
    if (!viewerIsGm(state)) {
      refuse("Only the DM (Owner or GM) may change a scene's light");
    }
    scene.ambientLight = level;
    scene.updatedAt = now();
    // The scene leads; its entry level is the same board.
    for (const row of state.levels) {
      if (row.sceneId === sceneId && row.isEntry) row.ambientLight = level;
    }
    record(EVENT.sceneLighting, {
      action: "changed",
      sceneId,
      ambientLight: level,
    });
    markChanged();
    return scene;
  },

  setSceneExploration: ({ sceneId, enabled }) => {
    const state = demoState();
    gmOnlyExploration(state, sceneId);
    explorationOf(state, sceneId).enabled = enabled;
    markChanged();
    return enabled;
  },

  resetSceneExploration: ({ sceneId, forUser }) => {
    const state = demoState();
    gmOnlyExploration(state, sceneId);
    const scene = explorationOf(state, sceneId);
    let epoch: number;
    if (forUser == null) {
      // Everyone: past every number already handed out, and the per-member
      // rows go, since the scene's own number now outranks them all.
      epoch = Math.max(scene.epoch, ...Object.values(scene.resets)) + 1;
      scene.epoch = epoch;
      scene.resets = {};
    } else {
      const own = scene.resets[forUser] ?? scene.epoch;
      epoch = Math.max(own, scene.epoch) + 1;
      scene.resets[forUser] = epoch;
    }
    record(EVENT.explorationReset, {
      action: "reset",
      sceneId,
      forUser: forUser ?? null,
      epoch,
    });
    markChanged();
    return epoch;
  },
};
