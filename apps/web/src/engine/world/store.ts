import type {
  EngineCommandSource,
  WorldCommand,
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

    // create_wall/update_wall/delete_wall are intents, not confirmed
    // state: a sync-layer subscriber (engine/world/sync/walls.ts) turns
    // them into GraphQL mutations and dispatches upsert_wall/remove_wall
    // once the server confirms. They pass through the store unchanged so
    // both the sync subscriber and the Bevy bridge can observe them.
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
