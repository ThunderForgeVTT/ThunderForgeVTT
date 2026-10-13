/**
 * The Bevy bridge's public entry. Everything lives in a sibling module by
 * responsibility and is re-exported here, so callers import `@/engine/bevy`
 * and never reach past it:
 *
 * - `wasmModule`: loading the wasm bundle, with its progress
 * - `mount`: WebGL2 checks, mounting, and the engine's lifecycle state
 * - `engineEvents`: the reports the engine sends chrome, and their listeners
 * - `worldBridge`: binding the world store to the engine, both directions
 * - `authoring`, `viewer`, `camera`, `dice`: commands across the boundary
 * - `worldCache`, `peer`: the browser cache, its outbox and peer transfer
 * - `engineProbe`: the development-only `window.__engineProbe`
 */
export type {
  EngineLoadListener,
  EngineLoadProgress,
  EngineLoadStage,
} from "./wasmModule";
export {
  boardSizeCeiling,
  getEngineState,
  mountEngine,
  setActiveWorld,
  webgl2Unavailable,
} from "./mount";
export type {
  AuthoringToolRevokedEvent,
  CanvasContextMenuEvent,
  DoorIconPressedEvent,
  GridSnapChangedEvent,
  InteractionTriggeredEvent,
  InteractionUnavailableEvent,
  MovementBlockedEvent,
  OpenLoreEvent,
  PickUpItemEvent,
  PlacementConfirmedEvent,
  SceneLoadRequestedEvent,
} from "./engineEvents";
export {
  onAuthoringToolRevoked,
  onCanvasContextMenu,
  onDoorIconPressed,
  onGridSnapChanged,
  onInteractionTriggered,
  onInteractionUnavailable,
  onMovementBlocked,
  onOpenLore,
  onPickUpItem,
  onPlacementCancelled,
  onPlacementConfirmed,
  onSceneLoadRequested,
  onSdkError,
} from "./engineEvents";
export {
  bindWorldStore,
  getBoundWorldStore,
  unmountEngine,
} from "./worldBridge";
export { boardCentre, boardPointToClient, getCameraState } from "./camera";
export {
  beginPropPlacement,
  beginSceneTransition,
  beginTokenPlacement,
  cancelTokenPlacement,
  clearAllowedAuthoringTools,
  completeSceneTransition,
  getAuthoringMode,
  setActiveShapeTool,
  setAllowedAuthoringTools,
  setAuthoringMode,
  setGridSnap,
  setSelectionFilter,
  setWallPrimitive,
} from "./authoring";
export {
  exploredCells,
  setControlledToken,
  setDisplayAppearance,
  setExploration,
  setExploredCells,
  setIsGameMaster,
  setPartyEyes,
  setViewerToken,
  setViewerUser,
} from "./viewer";
export { setReducedMotion, triggerDiceRollAnimation } from "./dice";
export type { QueuedChangeWire, WorldCacheSyncSummary } from "./worldCache";
export {
  forgetReconciledChanges,
  queueOfflineChange,
  readQueuedChanges,
  syncWorldCache,
} from "./worldCache";
export {
  beginPeerAdjudication,
  endPeerAdjudication,
  isPeerTransferRunning,
  peerAdjudicationActive,
  peerAdjudicationSubmissions,
  proposeTokenTransform,
  startPeerTransfer,
  stopPeerTransfer,
} from "./peer";
