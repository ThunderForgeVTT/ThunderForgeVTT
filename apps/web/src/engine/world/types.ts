export type EngineCommandSource = "bevy" | "tldraw" | "ui" | "sync";

export type WorldToken = {
  id: string;
  x: number;
  y: number;
  z: number;
  label?: string;
};

export type DoorState = "none" | "open" | "closed";

// Confirmed wall state, as it exists in the world store (mirrors the
// server's Wall/GraphQLWall shape once a create/update has round-tripped).
export type WorldWall = {
  id: string;
  sceneId: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  blocksVision: boolean;
  blocksMovement: boolean;
  doorState: DoorState;
};

export type WorldState = {
  worldId: string;
  tokens: Record<string, WorldToken>;
  walls: Record<string, WorldWall>;
  selectedWallId: string | null;
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

// Confirmed wall upsert/remove: dispatched once a wall's state is known
// (after a successful mutation response, or after a world_events NOTIFY
// refetch). Consumed by the reducer and forwarded to the engine the same
// way upsert_token/remove_token are.
export type UpsertWallCommand = {
  type: "upsert_wall";
  wall: WorldWall;
};

export type RemoveWallCommand = {
  type: "remove_wall";
  wallId: string;
};

export type SelectWallCommand = {
  type: "select_wall";
  wallId: string | null;
};

export type WallFieldChanges = Partial<{
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  blocksVision: boolean;
  blocksMovement: boolean;
  doorState: DoorState;
}>;

// Wall *intents*: requests to create/update/delete a wall that have not
// yet round-tripped the server. These can originate from the UI (the
// WallTool property panel) or from the Bevy engine (in-canvas drawing/
// editing, per the engine's apply_world_command contract). A subscriber
// in engine/world/sync/walls.ts turns these into GraphQL mutation calls
// and, on success, dispatches the confirmed upsert_wall/remove_wall
// commands above.
export type CreateWallCommand = {
  type: "create_wall";
  wall: {
    x1: number;
    y1: number;
    x2: number;
    y2: number;
    blocksVision: boolean;
    blocksMovement: boolean;
    doorState: DoorState;
  };
  worldId?: string;
};

export type UpdateWallCommand = {
  type: "update_wall";
  wallId: string;
  changes: WallFieldChanges;
  worldId?: string;
};

export type DeleteWallCommand = {
  type: "delete_wall";
  wallId: string;
  worldId?: string;
};

export type WorldCommand =
  | SetWorldCommand
  | UpsertTokenCommand
  | RemoveTokenCommand
  | UpsertWallCommand
  | RemoveWallCommand
  | SelectWallCommand
  | CreateWallCommand
  | UpdateWallCommand
  | DeleteWallCommand;

export type WorldStoreEvent = {
  command: WorldCommand;
  state: WorldState;
  source: EngineCommandSource;
};

export type WorldStoreSubscriber = (event: WorldStoreEvent) => void;
