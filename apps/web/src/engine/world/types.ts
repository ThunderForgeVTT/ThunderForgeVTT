export type EngineCommandSource = "bevy" | "ui" | "sync";

export type WorldToken = {
  id: string;
  x: number;
  y: number;
  z: number;
  label?: string;
  // Spec 004: canvas-native resize/rotate + per-player ownership.
  rotation?: number;
  scale?: number;
  ownerUserId?: string | null;
  isPrimary?: boolean;
  photoUrl?: string | null;
  health?: number | null;
  maxHealth?: number | null;
  /**
   * What the token represents, deciding the colour the engine draws it in
   * when it carries no art. Optional because older payloads predate it; the
   * engine treats a missing value as `character`.
   */
  tokenType?: string | null;
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

// Confirmed light source state, as it exists in the world store (mirrors
// the server's LightSource/GraphQLLightSource shape once a create/update
// has round-tripped).
export type WorldLight = {
  id: string;
  sceneId: string;
  x: number;
  y: number;
  radius: number;
  intensity: number;
  color: string | null;
  attachedTokenId: string | null;
  castsShadows: boolean;
};

export type ShapeKind = "stroke" | "rect" | "ellipse" | "line" | "text";

// Confirmed shape state, as it exists in the world store (mirrors the
// server's Shape/GraphQLShape shape once a create/update has
// round-tripped). `geometry` and `style` are opaque JSON blobs — the
// engine (Bevy) interprets `geometry`'s contents per `kind`; this store
// never inspects them.
export type WorldShape = {
  id: string;
  sceneId: string;
  kind: ShapeKind;
  geometry: Record<string, unknown>;
  text: string | null;
  style: Record<string, unknown> | null;
  visibleToPlayers: boolean;
};

export type WorldState = {
  worldId: string;
  tokens: Record<string, WorldToken>;
  selectedTokenId: string | null;
  /**
   * The full selection, topmost first, when a click resolved to a stack.
   * `selectedTokenId` is always its first element, so single-selection
   * consumers can keep reading that and ignore this entirely.
   */
  selectedTokenIds: string[];
  walls: Record<string, WorldWall>;
  selectedWallId: string | null;
  lights: Record<string, WorldLight>;
  selectedLightId: string | null;
  shapes: Record<string, WorldShape>;
  selectedShapeId: string | null;
};

export type SetWorldCommand = {
  type: "set_world";
  worldId: string;
};

// Switching scenes (SceneSwitcher) re-points the engine's background
// sprite at the newly-selected scene's imported map art (or clears it, if
// that scene has none). This is engine-signaling only — WorldState
// doesn't track it, `bindWorldStore`'s generic forwarder relays it to the
// engine's `set_scene_background` command the same way every other
// WorldCommand is relayed.
export type SetSceneBackgroundCommand = {
  type: "set_scene_background";
  backgroundImagePath: string | null;
  width: number;
  height: number;
  worldId?: string;
};

// The scene's grid — the lattice the engine snaps tokens to, measures
// movement in, and draws the overlay from. Sent alongside
// `set_scene_background` on every scene change, because the two must agree:
// an imported map's `pixels_per_grid` is what makes the drawn grid line up
// with the art beneath it. Engine-signaling only, like the background above.
export type SetSceneGridCommand = {
  type: "set_scene_grid";
  /** The scene's raw `gridType` ("square" | "hex" | "gridless"). */
  gridType: string;
  /** The scene's `gridSize` — centre-to-centre cell spacing in world units. */
  size: number;
  /** The map's extent. Given both, the engine anchors the grid to the map's
   * corner so it lands on the grid painted on the art. */
  mapWidth?: number;
  mapHeight?: number;
  /** Defaults to the world origin, where the background sprite is centred. */
  originX?: number;
  originY?: number;
  visible?: boolean;
};

// Emitted by the engine alongside `select_token` when a click resolves to
// one or more tokens. `select_token` still carries the primary, so existing
// single-selection consumers are untouched; this carries the whole stack,
// topmost first, for callers that understand stacking.
export type SelectTokensCommand = {
  type: "select_tokens";
  /** Topmost first. Empty means the click landed on empty canvas. */
  tokenIds: string[];
};

export type UpsertTokenCommand = {
  type: "upsert_token";
  token: WorldToken;
};

export type RemoveTokenCommand = {
  type: "remove_token";
  tokenId: string;
};

// Spec 004 (US2, T020): the engine emits this whenever a token is
// selected/deselected on the canvas (mirroring `select_wall`'s identical
// convention), so `TokenTool.tsx`'s resize/rotate panel knows which token
// (if any) to show controls for.
export type SelectTokenCommand = {
  type: "select_token";
  tokenId: string | null;
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

// Confirmed light upsert/remove: dispatched once a light's state is known
// (after a successful mutation response, or after a world_events NOTIFY
// refetch). Consumed by the reducer and forwarded to the engine the same
// way upsert_wall/remove_wall are.
export type UpsertLightCommand = {
  type: "upsert_light";
  light: WorldLight;
};

export type RemoveLightCommand = {
  type: "remove_light";
  lightId: string;
};

export type SelectLightCommand = {
  type: "select_light";
  lightId: string | null;
};

export type LightFieldChanges = Partial<{
  x: number;
  y: number;
  radius: number;
  intensity: number;
  color: string | null;
  attachedTokenId: string | null;
  castsShadows: boolean;
}>;

// Light *intents*: requests to create/update/delete a light source that
// have not yet round-tripped the server. These can originate from the UI
// (the LightingTool property panel) or from the Bevy engine (in-canvas
// placement/editing, per the engine's apply_world_command contract). A
// subscriber in engine/world/sync/lights.ts turns these into GraphQL
// mutation calls and, on success, dispatches the confirmed
// upsert_light/remove_light commands above.
export type CreateLightCommand = {
  type: "create_light";
  light: {
    x: number;
    y: number;
    radius: number;
    intensity: number;
    color: string | null;
    attachedTokenId: string | null;
    castsShadows: boolean;
  };
  worldId?: string;
};

export type UpdateLightCommand = {
  type: "update_light";
  lightId: string;
  changes: LightFieldChanges;
  worldId?: string;
};

export type DeleteLightCommand = {
  type: "delete_light";
  lightId: string;
  worldId?: string;
};

// Confirmed shape upsert/remove: dispatched once a shape's state is known
// (after a successful mutation response, or after a world_events NOTIFY
// refetch). Consumed by the reducer and forwarded to the engine the same
// way upsert_wall/remove_wall does for walls.
export type UpsertShapeCommand = {
  type: "upsert_shape";
  shape: WorldShape;
};

export type RemoveShapeCommand = {
  type: "remove_shape";
  shapeId: string;
};

export type SelectShapeCommand = {
  type: "select_shape";
  shapeId: string | null;
};

export type ShapeFieldChanges = Partial<{
  geometry: Record<string, unknown>;
  text: string | null;
  style: Record<string, unknown> | null;
  visibleToPlayers: boolean;
}>;

// Shape *intents*: requests to create/update/delete a shape that have not
// yet round-tripped the server. These can originate from the UI (the
// ShapeTool toolbar/style panel) or from the Bevy engine (in-canvas
// drawing, per the engine's apply_world_command contract). A subscriber
// in engine/world/sync/shapes.ts turns these into GraphQL mutation calls
// and, on success, dispatches the confirmed upsert_shape/remove_shape
// commands above.
export type CreateShapeCommand = {
  type: "create_shape";
  shape: {
    kind: ShapeKind;
    geometry: Record<string, unknown>;
    text?: string | null;
    style?: Record<string, unknown> | null;
    visibleToPlayers: boolean;
  };
  worldId?: string;
};

export type UpdateShapeCommand = {
  type: "update_shape";
  shapeId: string;
  changes: ShapeFieldChanges;
  worldId?: string;
};

export type DeleteShapeCommand = {
  type: "delete_shape";
  shapeId: string;
  worldId?: string;
};

// Spec 002 (US3): a pasted (or, latently, migrated-background) canvas
// image asset. Not tracked in WorldState (no reducer case — see
// store.ts's default case) since nothing currently reads a
// canvas-image-asset slice of store state; bindWorldStore's generic
// forwarder still relays this to the engine's apply_world_command the
// same way create_wall/etc. intents are, which is all spawning the
// placed-image sprite needs (src/engine/src/systems/background.rs's
// sync_placed_canvas_images).
export type UpsertCanvasImageAssetCommand = {
  type: "upsert_canvas_image_asset";
  assetId: string;
  path: string;
  x: number;
  y: number;
  width: number;
  height: number;
};

export type RemoveCanvasImageAssetCommand = {
  type: "remove_canvas_image_asset";
  assetId: string;
};

export type WorldCommand =
  | SetWorldCommand
  | SetSceneBackgroundCommand
  | SetSceneGridCommand
  | UpsertTokenCommand
  | RemoveTokenCommand
  | SelectTokenCommand
  | SelectTokensCommand
  | UpsertWallCommand
  | RemoveWallCommand
  | SelectWallCommand
  | CreateWallCommand
  | UpdateWallCommand
  | DeleteWallCommand
  | UpsertLightCommand
  | RemoveLightCommand
  | SelectLightCommand
  | CreateLightCommand
  | UpdateLightCommand
  | DeleteLightCommand
  | UpsertShapeCommand
  | RemoveShapeCommand
  | SelectShapeCommand
  | CreateShapeCommand
  | UpdateShapeCommand
  | DeleteShapeCommand
  | UpsertCanvasImageAssetCommand
  | RemoveCanvasImageAssetCommand;

export type WorldStoreEvent = {
  command: WorldCommand;
  state: WorldState;
  source: EngineCommandSource;
};

export type WorldStoreSubscriber = (event: WorldStoreEvent) => void;
