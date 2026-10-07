/**
 * Scenes: made, changed, hidden, and filled with the party, as the server
 * does it.
 *
 * Mirrors `graphql/mutations_scenes.rs` (`create_scene`, `update_scene`,
 * `update_scene_hidden_impl`), `graphql/mutations_party.rs`
 * (`bring_party_to_scene_impl`) and, for who sees what, `scenes_impl` and
 * `scene_impl` in `graphql/queries/scene.rs`. A new scene gets its entry
 * level, "Ground", as the `scenes_create_entry_level_trigger` gives it one,
 * and a change to the scene's board is carried to that level as
 * `scenes_mirror_board_trigger` carries it.
 */
import { DEMO_PLAYER, DEMO_USER, newToken, tokenOf } from "../../seed/world";
import { viewerIsGm } from "../actors";
import { EVENT, record } from "../events";
import { demoState, markChanged, type DemoState, type Row } from "../state";
import { given, levelsOf, refuse, type Handler } from "./common";

function now(): string {
  return new Date().toISOString();
}

/** The server renders markdown; the demo escapes it and keeps the lines. */
function rendered(markdown: string): string {
  const text = markdown.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
  return text.trim() ? `<p>${text}</p>\n` : "";
}

/**
 * `scenes_impl` and `scene_impl`: a Game Master sees every scene; anyone
 * else the ones not hidden, and the one the world is playing.
 */
function sees(state: DemoState, scene: Row): boolean {
  return (
    viewerIsGm(state) ||
    scene.hidden !== true ||
    scene.sceneId === state.world.activeSceneId
  );
}

/** The trigger's mirror: the scene's board is its entry level's. */
export function mirrorToEntry(state: DemoState, scene: Row): void {
  const entry = levelsOf(state, scene.sceneId as string).find((l) => l.isEntry);
  if (!entry) return;
  entry.backgroundAssetId = scene.backgroundAssetId;
  entry.backgroundUrl = scene.backgroundUrl;
  entry.width = scene.width;
  entry.height = scene.height;
  entry.ambientLight = scene.ambientLight;
}

function createScene(input: Row): Row {
  const state = demoState();
  if (!viewerIsGm(state) || input.worldId !== state.world.id) {
    refuse("Forbidden");
  }
  const at = now();
  const scene: Row = {
    sceneId: crypto.randomUUID(),
    worldId: state.world.id,
    name: input.name,
    description: input.description ?? null,
    type: input.type ?? "battlemap",
    gridSize: input.gridSize ?? 5,
    gridType: input.gridType ?? state.world.defaultSceneGridType ?? "square",
    width: input.width ?? 100,
    height: input.height ?? 100,
    metadata: input.metadata ?? null,
    ownerId: DEMO_USER.id,
    createdAt: at,
    updatedAt: at,
    backgroundImagePath: null,
    backgroundAssetId: null,
    backgroundUrl: null,
    summaryMarkdown: null,
    summaryRenderedHtml: null,
    // Spec 022 FR-003: every new scene starts hidden.
    hidden: true,
    previewUrl: null,
    backgroundGridMismatch: null,
    ambientLight: "bright",
  };
  state.scenes.push(scene);
  state.levels.push({
    levelId: crypto.randomUUID(),
    sceneId: scene.sceneId,
    name: "Ground",
    sortOrder: 0,
    isEntry: true,
    hidden: false,
    backgroundAssetId: null,
    backgroundUrl: null,
    width: scene.width,
    height: scene.height,
    ambientLight: scene.ambientLight,
  });
  markChanged();
  return scene;
}

function updateScene(sceneId: string, input: Row): Row {
  const state = demoState();
  const scene = state.scenes.find((s) => s.sceneId === sceneId);
  // `is_dm_of_scene` false is answered as the update failing.
  if (!scene || !viewerIsGm(state)) refuse("Failed to update scene");
  const changes = given({
    name: input.name,
    description: input.description,
    gridSize: input.gridSize,
    gridType: input.gridType,
    width: input.width,
    height: input.height,
    metadata: input.metadata,
  });
  Object.assign(scene, changes, { updatedAt: now() });
  if (typeof input.summaryMarkdown === "string") {
    scene.summaryMarkdown = input.summaryMarkdown;
    scene.summaryRenderedHtml = rendered(input.summaryMarkdown);
  }
  if ("width" in changes || "height" in changes) mirrorToEntry(state, scene);
  markChanged();
  return scene;
}

function updateSceneHidden(sceneId: string, hidden: boolean): Row {
  const state = demoState();
  const scene =
    state.scenes.find((s) => s.sceneId === sceneId) ??
    refuse("Scene not found");
  if (!viewerIsGm(state)) {
    refuse("Only the DM (Owner or GM) may change a scene's visibility");
  }
  scene.hidden = hidden;
  markChanged();
  return scene;
}

/** `arrival_position`: five to a row, a grid step apart. */
function arrivalPosition(index: number, gridSize: number): [number, number] {
  const step = gridSize > 0 ? gridSize : 5;
  return [step * ((index % 5) + 1), step * (Math.floor(index / 5) + 1)];
}

/** `arrival_owner`: the player who claimed the character, else its owner. */
function arrivalOwner(state: DemoState, actor: Row): string {
  return state.claimedActorId === actor.id
    ? DEMO_PLAYER.id
    : (actor.ownedBy as string);
}

function bringPartyToScene(input: Row): Row {
  const state = demoState();
  const sceneId = input.sceneId as string;
  const scene =
    state.scenes.find((s) => s.sceneId === sceneId) ??
    refuse("Scene not found");
  if (!viewerIsGm(state)) {
    refuse("Only the DM (Owner or GM) may bring the party");
  }
  const requested = (input.actorIds as string[] | null | undefined) ?? [];
  const selection =
    requested.length > 0
      ? requested
      : state.actors
          .filter((a) => !a.isNpc)
          .sort(
            (a, b) =>
              String(a.createdAt).localeCompare(String(b.createdAt)) ||
              String(a.id).localeCompare(String(b.id)),
          )
          .map((a) => a.id as string);
  const party = new Map(
    state.actors.filter((a) => !a.isNpc).map((a) => [a.id as string, a]),
  );
  const stranger = selection.find((id) => !party.has(id));
  if (stranger) {
    refuse(`Character ${stranger} is not a player character in this world`);
  }
  const occupants = new Set(
    state.tokens
      .filter((t) => t.sceneId === sceneId && t.actorId != null)
      .map((t) => t.actorId as string),
  );
  const alreadyPresent = [
    ...new Set(selection.filter((id) => occupants.has(id))),
  ];
  const toCreate = [...new Set(selection.filter((id) => !occupants.has(id)))];
  const entry = levelsOf(state, sceneId).find((l) => l.isEntry);
  const at = now();
  toCreate.forEach((actorId, index) => {
    const actor = party.get(actorId) as Row;
    const [x, y] = arrivalPosition(index, scene.gridSize as number);
    state.tokens.push(
      newToken({
        tokenId: crypto.randomUUID(),
        sceneId,
        levelId: entry?.levelId as string,
        x,
        y,
        at,
        rest: {
          ...tokenOf(actor),
          tokenType: "character",
          linked: true,
          ownerUserId: arrivalOwner(state, actor),
        },
      }),
    );
  });
  if (toCreate.length > 0) {
    // One announcement for the arrival, as the server makes one.
    record(EVENT.token, {
      action: "created",
      scene_id: sceneId,
      reason: "party_arrived",
      actor_ids: toCreate,
    });
    markChanged();
  }
  return {
    sceneId,
    arrivedActorIds: toCreate,
    alreadyPresentActorIds: alreadyPresent,
  };
}

export const sceneQueries: Record<string, Handler> = {
  scenes: () => {
    const state = demoState();
    return state.scenes.filter((s) => sees(state, s));
  },
  scene: ({ sceneId }) => {
    const state = demoState();
    const scene = state.scenes.find((s) => s.sceneId === sceneId);
    return scene && sees(state, scene) ? scene : null;
  },
};

export const sceneMutations: Record<string, Handler> = {
  createScene: ({ input }) => createScene(input),
  updateScene: ({ sceneId, input }) => updateScene(sceneId, input),
  updateSceneHidden: ({ sceneId, hidden }) =>
    updateSceneHidden(sceneId, hidden),
  bringPartyToScene: ({ input }) => bringPartyToScene(input),
};
