import type {
  EngineCommandSource,
  WorldCommand,
  WorldLight,
  WorldState,
  WorldStoreEvent,
  WorldStoreSubscriber,
  WorldToken,
  WorldWall,
} from "./types";

type CreateWorldStoreOptions = {
  worldId: string;
  initialTokens?: WorldToken[];
  initialWalls?: WorldWall[];
  initialLights?: WorldLight[];
};

export type WorldStore = {
  getState: () => Readonly<WorldState>;
  dispatch: (command: WorldCommand, source?: EngineCommandSource) => void;
  subscribe: (subscriber: WorldStoreSubscriber) => () => void;
};

function normalizeTokens(tokens: WorldToken[]): Record<string, WorldToken> {
  const byId: Record<string, WorldToken> = {};

  for (const token of tokens) {
    byId[token.id] = token;
  }

  return byId;
}

function normalizeWalls(walls: WorldWall[]): Record<string, WorldWall> {
  const byId: Record<string, WorldWall> = {};

  for (const wall of walls) {
    byId[wall.id] = wall;
  }

  return byId;
}

function normalizeLights(lights: WorldLight[]): Record<string, WorldLight> {
  const byId: Record<string, WorldLight> = {};

  for (const light of lights) {
    byId[light.id] = light;
  }

  return byId;
}

function reduceState(state: WorldState, command: WorldCommand): WorldState {
  switch (command.type) {
    case "set_world":
      return {
        ...state,
        worldId: command.worldId,
      };

    case "upsert_token":
      return {
        ...state,
        tokens: {
          ...state.tokens,
          [command.token.id]: command.token,
        },
      };

    case "remove_token": {
      const nextTokens = { ...state.tokens };
      delete nextTokens[command.tokenId];

      return {
        ...state,
        tokens: nextTokens,
      };
    }

    case "upsert_wall":
      return {
        ...state,
        walls: {
          ...state.walls,
          [command.wall.id]: command.wall,
        },
      };

    case "remove_wall": {
      const nextWalls = { ...state.walls };
      delete nextWalls[command.wallId];

      return {
        ...state,
        walls: nextWalls,
        selectedWallId:
          state.selectedWallId === command.wallId ? null : state.selectedWallId,
      };
    }

    case "select_wall":
      return {
        ...state,
        selectedWallId: command.wallId,
      };

    case "upsert_light":
      return {
        ...state,
        lights: {
          ...state.lights,
          [command.light.id]: command.light,
        },
      };

    case "remove_light": {
      const nextLights = { ...state.lights };
      delete nextLights[command.lightId];

      return {
        ...state,
        lights: nextLights,
        selectedLightId:
          state.selectedLightId === command.lightId ? null : state.selectedLightId,
      };
    }

    case "select_light":
      return {
        ...state,
        selectedLightId: command.lightId,
      };

    // create_wall/update_wall/delete_wall and create_light/update_light/
    // delete_light are intents, not confirmed state: a sync-layer
    // subscriber (engine/world/sync/walls.ts, engine/world/sync/lights.ts)
    // turns them into GraphQL mutations and dispatches
    // upsert_wall/remove_wall or upsert_light/remove_light once the
    // server confirms. They pass through the store unchanged so both the
    // sync subscriber and the Bevy bridge can observe them.
    default:
      return state;
  }
}

export function createWorldStore(options: CreateWorldStoreOptions): WorldStore {
  let state: WorldState = {
    worldId: options.worldId,
    tokens: normalizeTokens(options.initialTokens ?? []),
    walls: normalizeWalls(options.initialWalls ?? []),
    selectedWallId: null,
    lights: normalizeLights(options.initialLights ?? []),
    selectedLightId: null,
  };

  const subscribers = new Set<WorldStoreSubscriber>();

  function emit(event: WorldStoreEvent) {
    for (const subscriber of subscribers) {
      subscriber(event);
    }
  }

  return {
    getState() {
      return state;
    },

    dispatch(command, source = "ui") {
      state = reduceState(state, command);
      emit({ command, state, source });
    },

    subscribe(subscriber) {
      subscribers.add(subscriber);
      return () => subscribers.delete(subscriber);
    },
  };
}
