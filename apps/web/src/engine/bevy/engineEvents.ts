import type { EngineSdkError } from "@/engine/sdk/commands";

/**
 * Everyone listening for commands the engine refused.
 *
 * A refusal that reaches nobody is indistinguishable from the silent drop the
 * SDK version stamp exists to retire, so the report needs somewhere to land.
 * Listeners are notified; they cannot inject, which keeps this an observation
 * surface rather than a way to fake engine failures in a test.
 */
const sdkErrorListeners = new Set<(error: EngineSdkError) => void>();

/** Subscribe to SDK errors. Returns the unsubscribe. */
export function onSdkError(
  listener: (error: EngineSdkError) => void,
): () => void {
  sdkErrorListeners.add(listener);
  return () => {
    sdkErrorListeners.delete(listener);
  };
}

export function reportSdkError(error: EngineSdkError): void {
  // Logged as well as dispatched: a bundle drift that nobody has subscribed
  // to still needs to be visible to whoever is looking at the console.
  console.error(
    `[engine sdk] ${error.code}: ${error.message}`,
    error.command ?? "",
  );
  for (const listener of sdkErrorListeners) {
    try {
      listener(error);
    } catch {
      // One bad listener must not stop the others hearing about this.
    }
  }
}

/**
 * The engine asking for a lore entry to be opened.
 *
 * Spec 030, US1. The engine recognises the effect and resolves which entry it
 * points at; opening a tab needs the application's URL structure, which is
 * chrome and belongs on this side (Constitution Principle I).
 */
export interface OpenLoreEvent {
  type: "openLore";
  interactiveId: string;
  entryId: string;
}

export function asOpenLore(event: unknown): OpenLoreEvent | null {
  if (typeof event !== "object" || event === null) return null;
  const candidate = event as { type?: unknown };
  return candidate.type === "openLore" ? (event as OpenLoreEvent) : null;
}

/** A player asked to pick a placed item up. Chrome asks the server. */
export interface PickUpItemEvent {
  type: "pickUpItem";
  interactiveId: string;
  itemId: string;
  /** The scene token to remove once the server agrees. */
  subjectRef?: string;
}

export function asPickUpItem(event: unknown): PickUpItemEvent | null {
  const candidate = event as { type?: unknown };
  return candidate.type === "pickUpItem" ? (event as PickUpItemEvent) : null;
}

/**
 * Spec 045 FR-013/FR-015: a move the engine refused, because a wall is in
 * the way.
 *
 * Not a world command — nothing changed, which is the point. The engine has
 * already stopped the token; this exists so the player is told *why* it did
 * not move, since a token that simply does not move is indistinguishable from
 * a key that did not register.
 *
 * Names the wall and never what kind of wall it is: a closed secret door stops
 * a player like any other, and this must not be how they learn there is a door
 * there (FR-019).
 */
export interface MovementBlockedEvent {
  type: "movement_blocked";
  /** The token refused. */
  tokenId?: string;
  wallId: string;
  at: { x: number; y: number };
}

/**
 * The engine's snapping switch moved (spec 077). Fired for every change —
 * the `S` key, `setGridSnap`, the first frame — so the rail's button reads
 * the engine's state rather than remembering its own.
 */
export interface GridSnapChangedEvent {
  type: "grid_snap_changed";
  enabled: boolean;
}

export function asGridSnapChanged(event: unknown): GridSnapChangedEvent | null {
  const candidate = event as { type?: unknown; enabled?: unknown };
  return candidate.type === "grid_snap_changed" &&
    typeof candidate.enabled === "boolean"
    ? (event as GridSnapChangedEvent)
    : null;
}

export function asMovementBlocked(event: unknown): MovementBlockedEvent | null {
  const candidate = event as { type?: unknown };
  return candidate.type === "movement_blocked"
    ? (event as MovementBlockedEvent)
    : null;
}

/**
 * An interactive whose effect this build cannot perform.
 *
 * ADR-054: absence is detected *before* dispatch, by comparing the stored
 * effect id against the assembled registry — never by noticing that a
 * fire-and-forget dispatch did nothing. A Game Master whose build lacks the
 * subsystem has a switch they cannot use, not a scene that failed to load.
 */
export interface InteractionUnavailableEvent {
  type: "interactionUnavailable";
  interactiveId: string;
  effectId: string;
}

export function asInteractionUnavailable(
  event: unknown,
): InteractionUnavailableEvent | null {
  const candidate = event as { type?: unknown };
  return candidate.type === "interactionUnavailable"
    ? (event as InteractionUnavailableEvent)
    : null;
}

/**
 * A tool was taken away while this person was holding it.
 *
 * The engine has already dropped them back to Select and discarded whatever
 * gesture was in flight — that part is not chrome's decision. What chrome owes
 * is legibility: spec 031's edge case is explicit that the tool must not
 * "silently cease to respond", which is precisely what a mid-gesture
 * revocation looks like without this.
 */
export interface AuthoringToolRevokedEvent {
  type: "authoringToolRevoked";
  /** The tool that was being used when it was lost. */
  tool: string;
}

export function asAuthoringToolRevoked(
  event: unknown,
): AuthoringToolRevokedEvent | null {
  const candidate = event as { type?: unknown };
  return candidate.type === "authoringToolRevoked"
    ? (event as AuthoringToolRevokedEvent)
    : null;
}

export const authoringToolRevokedListeners = new Set<
  (event: AuthoringToolRevokedEvent) => void
>();

/** Be told when a tool was revoked mid-use. Returns the unsubscribe. */
export function onAuthoringToolRevoked(
  listener: (event: AuthoringToolRevokedEvent) => void,
): () => void {
  authoringToolRevokedListeners.add(listener);
  return () => {
    authoringToolRevokedListeners.delete(listener);
  };
}

/**
 * A right-click on the play field, and what was under it (spec 031 FR-029).
 *
 * The engine reports the gesture and the tokens its hit test found — topmost
 * first, and only tokens this board draws — and nothing else. The menu is
 * chrome's (Constitution Principle I): what may be done, and to what, is
 * decided in React against what the server allows this viewer.
 *
 * `screenX`/`screenY` are the pointer in CSS pixels from the canvas's own
 * top-left corner, where the press was; `worldX`/`worldY` are the same point
 * on the board. An empty `tokenIds` is a right-click on bare board.
 *
 * `wallId` is the wall or door within reach of the pointer (spec 071): any
 * wall for a Game Master, and for anybody else only a door they are shown.
 */
export interface CanvasContextMenuEvent {
  type: "canvas_context_menu";
  worldX: number;
  worldY: number;
  screenX: number;
  screenY: number;
  tokenIds: string[];
  wallId?: string | null;
  /** The placed light under the pointer; a Game Master's only (spec 073). */
  lightId?: string | null;
  /** The drawing under the pointer; a Game Master's only (spec 073). */
  shapeId?: string | null;
}

export function asCanvasContextMenu(
  event: unknown,
): CanvasContextMenuEvent | null {
  const candidate = event as { type?: unknown; tokenIds?: unknown };
  return candidate.type === "canvas_context_menu" &&
    Array.isArray(candidate.tokenIds)
    ? (event as CanvasContextMenuEvent)
    : null;
}

export const canvasContextMenuListeners = new Set<
  (event: CanvasContextMenuEvent) => void
>();

/** Be told when the play field is right-clicked. */
export function onCanvasContextMenu(
  listener: (event: CanvasContextMenuEvent) => void,
): () => void {
  canvasContextMenuListeners.add(listener);
  return () => {
    canvasContextMenuListeners.delete(listener);
  };
}

/**
 * A door's icon was pressed (spec 071 US3). The engine draws the icon and
 * claims the press; what it means for this viewer is chrome's
 * (`doorIconAction`), as a right-click's menu is.
 */
export interface DoorIconPressedEvent {
  type: "door_icon_pressed";
  wallId: string;
  icon: "open" | "close" | "padlock";
}

export function asDoorIconPressed(event: unknown): DoorIconPressedEvent | null {
  const candidate = event as { type?: unknown; wallId?: unknown };
  return candidate.type === "door_icon_pressed" &&
    typeof candidate.wallId === "string"
    ? (event as DoorIconPressedEvent)
    : null;
}

export const doorIconListeners = new Set<
  (event: DoorIconPressedEvent) => void
>();

/** Be told when a door's icon is pressed. */
export function onDoorIconPressed(
  listener: (event: DoorIconPressedEvent) => void,
): () => void {
  doorIconListeners.add(listener);
  return () => {
    doorIconListeners.delete(listener);
  };
}

/**
 * A carry that ended, one way or the other.
 *
 * `kind` is the word chrome used when it asked for the carry — `actor` for a
 * token from the actors pane, `prop` for something authored in the
 * interactions panel — and `reference` is whatever it handed over with it.
 * The engine interprets neither: it reports where the drop happened and this
 * side decides what, if anything, comes to exist there.
 */
export interface PlacementConfirmedEvent {
  type: "token_placement_confirmed";
  kind: string;
  reference: string;
  x: number;
  y: number;
}

export function asPlacementConfirmed(
  event: unknown,
): PlacementConfirmedEvent | null {
  const candidate = event as { type?: unknown };
  return candidate.type === "token_placement_confirmed"
    ? (event as PlacementConfirmedEvent)
    : null;
}

export const placementConfirmedListeners = new Set<
  (event: PlacementConfirmedEvent) => void
>();
export const placementCancelledListeners = new Set<() => void>();

/**
 * Be told where a carried thing was dropped.
 *
 * Nothing exists yet when this fires. The engine has reported a position and
 * gone back to `Idle`; whether a token is created is the server's decision,
 * asked for from here (Constitution Principle I).
 */
export function onPlacementConfirmed(
  listener: (event: PlacementConfirmedEvent) => void,
): () => void {
  placementConfirmedListeners.add(listener);
  return () => {
    placementConfirmedListeners.delete(listener);
  };
}

/**
 * Be told that a carry ended with nothing placed.
 *
 * Subscribed to as well as the confirmation because a panel that only hears
 * about drops would go on claiming it is placing something after Escape, a
 * tool change or a scene change abandoned the carry — and the engine treats
 * all four identically, which is the point of its one `OnExit`.
 */
export function onPlacementCancelled(listener: () => void): () => void {
  placementCancelledListeners.add(listener);
  return () => {
    placementCancelledListeners.delete(listener);
  };
}

export const pickUpItemListeners = new Set<(event: PickUpItemEvent) => void>();
export const unavailableListeners = new Set<
  (event: InteractionUnavailableEvent) => void
>();

/** Subscribe to pickup requests. Returns the unsubscribe. */
export function onPickUpItem(
  listener: (event: PickUpItemEvent) => void,
): () => void {
  pickUpItemListeners.add(listener);
  return () => {
    pickUpItemListeners.delete(listener);
  };
}

/** Subscribe to unavailable-interaction reports. Returns the unsubscribe. */
export function onInteractionUnavailable(
  listener: (event: InteractionUnavailableEvent) => void,
): () => void {
  unavailableListeners.add(listener);
  return () => {
    unavailableListeners.delete(listener);
  };
}

export const movementBlockedListeners = new Set<
  (event: MovementBlockedEvent) => void
>();

/** Subscribe to moves the engine stopped at a wall. Returns the unsubscribe. */
export function onMovementBlocked(
  listener: (event: MovementBlockedEvent) => void,
): () => void {
  movementBlockedListeners.add(listener);
  return () => {
    movementBlockedListeners.delete(listener);
  };
}

export const gridSnapListeners = new Set<
  (event: GridSnapChangedEvent) => void
>();

/**
 * Subscribe to the snapping switch (spec 077 FR-001). Returns the
 * unsubscribe. The engine reports its state on its first frame, so a
 * listener attached before the engine starts hears the initial value too.
 */
export function onGridSnapChanged(
  listener: (event: GridSnapChangedEvent) => void,
): () => void {
  gridSnapListeners.add(listener);
  return () => {
    gridSnapListeners.delete(listener);
  };
}

const openLoreListeners = new Set<(event: OpenLoreEvent) => void>();

/**
 * Be told when an interactive wants a lore page opened.
 *
 * A listener rather than a direct `window.open` here, because a popup blocker
 * treats a call made outside a user gesture very differently from one made
 * inside it — and only the component that handled the click knows which it is.
 */
export function onOpenLore(
  listener: (event: OpenLoreEvent) => void,
): () => void {
  openLoreListeners.add(listener);
  return () => {
    openLoreListeners.delete(listener);
  };
}

export function reportOpenLore(event: OpenLoreEvent): void {
  for (const listener of openLoreListeners) {
    try {
      listener(event);
    } catch {
      // One bad listener must not stop the others hearing about this.
    }
  }
}

/**
 * The engine reporting that something was triggered without being clicked.
 *
 * Spec 030, US5. A token crossed into a region. The engine detects it because
 * it is the only thing that knows both where the token was and where it is
 * now — and it *reports* rather than performs, because whether the crossing is
 * permitted is the server's decision (Principle III, ADR-054).
 */
export interface InteractionTriggeredEvent {
  type: "interactionTriggered";
  /** The token that crossed in. */
  tokenId?: string;
  interactiveId: string;
  trigger: "enter";
}

export function asInteractionTriggered(
  event: unknown,
): InteractionTriggeredEvent | null {
  if (typeof event !== "object" || event === null) return null;
  const candidate = event as { type?: unknown };
  return candidate.type === "interactionTriggered"
    ? (event as InteractionTriggeredEvent)
    : null;
}

const triggerListeners = new Set<(event: InteractionTriggeredEvent) => void>();

/** Be told when the engine detected a trigger that nobody clicked. */
export function onInteractionTriggered(
  listener: (event: InteractionTriggeredEvent) => void,
): () => void {
  triggerListeners.add(listener);
  return () => {
    triggerListeners.delete(listener);
  };
}

export function reportInteractionTriggered(
  event: InteractionTriggeredEvent,
): void {
  for (const listener of triggerListeners) {
    try {
      listener(event);
    } catch {
      // One bad listener must not stop the others hearing about this.
    }
  }
}

/**
 * The engine asking for a scene's contents.
 *
 * Emitted by the scene-transition machine at the one moment loading is safe:
 * the previous scene's tokens, walls, lights and shapes are already off the
 * canvas, and the machine has settled into `Loading` to wait. Fetching
 * alongside `beginSceneTransition` instead would race the engine's despawn,
 * which runs on its own frame — and the frame usually wins, which is the
 * worst of the two outcomes because the times it loses look like a map that
 * randomly came up empty.
 */
export interface SceneLoadRequestedEvent {
  type: "scene_load_requested";
  sceneId: string;
}

export function asSceneLoadRequested(
  event: unknown,
): SceneLoadRequestedEvent | null {
  if (typeof event !== "object" || event === null) return null;
  const candidate = event as { type?: unknown };
  return candidate.type === "scene_load_requested"
    ? (event as SceneLoadRequestedEvent)
    : null;
}

const sceneLoadRequestedListeners = new Set<
  (event: SceneLoadRequestedEvent) => void
>();

/**
 * Be told when the engine has cleared the canvas and wants a scene's content.
 *
 * A listener rather than a promise from `beginSceneTransition`: the machine
 * can be sent somewhere else mid-transition, so "which scene was asked for"
 * has to travel with the event rather than be assumed from the call that
 * started it.
 */
export function onSceneLoadRequested(
  listener: (event: SceneLoadRequestedEvent) => void,
): () => void {
  sceneLoadRequestedListeners.add(listener);
  return () => {
    sceneLoadRequestedListeners.delete(listener);
  };
}

export function reportSceneLoadRequested(event: SceneLoadRequestedEvent): void {
  for (const listener of sceneLoadRequestedListeners) {
    try {
      listener(event);
    } catch {
      // One bad listener must not stop the others hearing about this.
    }
  }
}
