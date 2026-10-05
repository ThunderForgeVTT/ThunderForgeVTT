import type { TokenRecord } from "@/types/token";
import type { WorldWall } from "@/engine/world/types";

/**
 * What the play field's right-click menu offers, decided from who is asking
 * and what they clicked — nothing else.
 *
 * Kept apart from the menu so the rule can be read, and tested, as a rule: a
 * menu item a viewer is shown is one the server would let them carry out.
 *
 * - A **player** may attack another creature with their own character's
 *   attacks (`makeAttack` is the attacker's controller's, spec 046 C1). On
 *   their own token, or on bare board, a player has nothing to do here.
 * - A **Game Master** may damage or heal a creature (`changeHitPoints`),
 *   make its token its actor or a copy (`setTokenLink`) and, when the
 *   world's system declares any, set its conditions (`applyActorCondition`,
 *   `clearActorCondition`) — all only for a token standing for an actor —
 *   hide or show its name
 *   (`setTokenNameVisibility`) and remove it (`deleteToken`). On bare board,
 *   they may place a token or add a light there.
 *
 * On a wall or a door (spec 071), see `doorMenuActions`.
 */

/** One attack the viewer's character can make, from its sheet. */
export interface SheetAttack {
  abilityId: string;
  name: string;
}

export type CanvasMenuAction =
  | { kind: "attack"; attack: SheetAttack }
  | { kind: "damage" }
  | { kind: "heal" }
  | { kind: "link"; linked: boolean }
  | { kind: "conditions" }
  | { kind: "name"; hidden: boolean }
  | { kind: "remove" }
  | { kind: "place-token" }
  | { kind: "add-light" }
  | DoorMenuAction;

export type DoorMenuAction =
  | { kind: "door-state"; open: boolean }
  /** Said, not offered: the one thing a player may know about a locked door. */
  | { kind: "door-locked" }
  | { kind: "door-lock"; locked: boolean }
  | { kind: "door-gm-lock" }
  | { kind: "door-reveal" }
  | { kind: "door-designate"; isDoor: boolean };

export interface CanvasMenuViewer {
  isGameMaster: boolean;
  userId: string | null;
}

/** The token this viewer attacks from: their own, primary first. */
export function attackerTokenOf(
  tokens: TokenRecord[],
  userId: string | null,
): TokenRecord | null {
  if (!userId) return null;
  const mine = tokens.filter(
    (token) => token.ownerUserId === userId && token.actorId !== null,
  );
  return mine.find((token) => token.isPrimary) ?? mine[0] ?? null;
}

export function canvasMenuActions(options: {
  viewer: CanvasMenuViewer;
  /** The right-clicked token's row, or `null` for bare board. */
  target: TokenRecord | null;
  /** Whether the target's name is hidden from players right now. */
  nameHidden: boolean;
  /** The token the viewer attacks from, if they have one here. */
  attacker: TokenRecord | null;
  /** That token's character's attacks. */
  attacks: SheetAttack[];
  /** Whether the world's system declares any condition (spec 067). */
  systemHasConditions?: boolean;
}): CanvasMenuAction[] {
  const { viewer, target, nameHidden, attacker, attacks } = options;

  if (!target) {
    return viewer.isGameMaster
      ? [{ kind: "place-token" }, { kind: "add-light" }]
      : [];
  }

  if (viewer.isGameMaster) {
    const actions: CanvasMenuAction[] = [];
    if (target.actorId) {
      actions.push({ kind: "damage" }, { kind: "heal" });
      actions.push({ kind: "link", linked: !target.linked });
      if (options.systemHasConditions) actions.push({ kind: "conditions" });
    }
    actions.push({ kind: "name", hidden: !nameHidden });
    actions.push({ kind: "remove" });
    return actions;
  }

  if (!attacker || attacker.tokenId === target.tokenId) return [];
  return attacks.map((attack) => ({ kind: "attack", attack }));
}

/**
 * Shut, locked and hidden: to the table it is wall.
 *
 * Not a fourth state — the three things it is made of are stored apart (spec
 * 030), and this only names having all of them at once.
 */
export function isGmLocked(wall: WorldWall): boolean {
  return (
    wall.doorState === "closed" && wall.locked === true && wall.secret === true
  );
}

/**
 * What a right-click on a wall or a door offers (spec 071).
 *
 * - A **Game Master** on a door may open or shut it, lock or unlock it, lock
 *   it as a wall (`door-gm-lock`: shut, locked and hidden from the table),
 *   show a hidden one to the table, and make it an ordinary wall again. On a
 *   plain wall they may make it a door.
 * - A **player** on a door may open or shut it when its interactive lets them
 *   (`canOpen`), and is told a locked one is locked rather than offered
 *   something the server would refuse. The engine never reports them a plain
 *   wall or a hidden door; neither offers anything here if one arrives.
 */
export function doorMenuActions(options: {
  viewer: CanvasMenuViewer;
  wall: WorldWall;
  /** Whether this viewer's activation of the door would do anything. */
  canOpen: boolean;
}): DoorMenuAction[] {
  const { viewer, wall, canOpen } = options;
  const isDoor = wall.doorState !== "none";

  if (!viewer.isGameMaster) {
    if (!isDoor || wall.secret) return [];
    if (wall.locked) return [{ kind: "door-locked" }];
    return canOpen
      ? [{ kind: "door-state", open: wall.doorState !== "open" }]
      : [];
  }

  if (!isDoor) return [{ kind: "door-designate", isDoor: true }];

  const actions: DoorMenuAction[] = [
    { kind: "door-state", open: wall.doorState !== "open" },
    { kind: "door-lock", locked: !wall.locked },
  ];
  if (!isGmLocked(wall)) actions.push({ kind: "door-gm-lock" });
  if (wall.secret) actions.push({ kind: "door-reveal" });
  actions.push({ kind: "door-designate", isDoor: false });
  return actions;
}

/** What the menu calls the thing that was right-clicked. */
export function wallName(wall: WorldWall): string {
  if (wall.doorState === "none") return "Wall";
  return wall.secret ? "Hidden door" : "Door";
}

/** What a person reads on the item. */
export function actionLabel(action: CanvasMenuAction, name: string): string {
  switch (action.kind) {
    case "attack":
      return `Attack ${name} with ${action.attack.name}`;
    case "damage":
      return "Damage…";
    case "heal":
      return "Heal…";
    case "link":
      return action.linked
        ? "Link to its actor"
        : "Make it a copy of its actor";
    case "conditions":
      return "Conditions…";
    case "name":
      return action.hidden ? "Hide name from players" : "Show name to players";
    case "remove":
      return "Remove from the board…";
    case "place-token":
      return "Place a token here…";
    case "add-light":
      return "Add a light here";
    case "door-state":
      return action.open ? "Open" : "Close";
    case "door-locked":
      return "Locked";
    case "door-lock":
      return action.locked ? "Lock" : "Unlock";
    case "door-gm-lock":
      return "Lock as a wall";
    case "door-reveal":
      return "Show it to the table";
    case "door-designate":
      return action.isDoor ? "Make this a door" : "Make it an ordinary wall";
  }
}

/** A stable test id per item. */
export function actionTestId(action: CanvasMenuAction): string {
  return action.kind === "attack"
    ? `canvas-menu-attack-${action.attack.abilityId}`
    : `canvas-menu-${action.kind}`;
}
