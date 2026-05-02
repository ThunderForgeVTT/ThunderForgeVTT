import type {
  EngineCommandSource,
  WorldCommand,
  WorldState,
  WorldStoreEvent,
  WorldStoreSubscriber,
  WorldToken,
} from "./types";

type CreateWorldStoreOptions = {
  worldId: string;
  initialTokens?: WorldToken[];
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

    default:
      return state;
  }
}

export function createWorldStore(options: CreateWorldStoreOptions): WorldStore {
  let state: WorldState = {
    worldId: options.worldId,
    tokens: normalizeTokens(options.initialTokens ?? []),
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