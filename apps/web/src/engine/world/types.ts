export type EngineCommandSource = "bevy" | "tldraw" | "ui";

export type WorldToken = {
  id: string;
  x: number;
  y: number;
  z: number;
  label?: string;
};

export type WorldState = {
  worldId: string;
  tokens: Record<string, WorldToken>;
};

export type SetWorldCommand = {
  type: "set_world";
  worldId: string;
};

export type UpsertTokenCommand = {
  type: "upsert_token";
  token: WorldToken;
};

export type RemoveTokenCommand = {
  type: "remove_token";
  tokenId: string;
};

export type WorldCommand = SetWorldCommand | UpsertTokenCommand | RemoveTokenCommand;

export type WorldStoreEvent = {
  command: WorldCommand;
  state: WorldState;
  source: EngineCommandSource;
};

export type WorldStoreSubscriber = (event: WorldStoreEvent) => void;