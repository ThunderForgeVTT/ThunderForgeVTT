import type { TokenRecord } from "@/types/token";

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
 *   make its token its actor or a copy (`setTokenLink`) — both only for a
 *   token standing for an actor — hide or show its name
 *   (`setTokenNameVisibility`) and remove it (`deleteToken`). On bare board,
 *   they may place a token or add a light there.
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
  | { kind: "name"; hidden: boolean }
  | { kind: "remove" }
  | { kind: "place-token" }
  | { kind: "add-light" };

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
    }
    actions.push({ kind: "name", hidden: !nameHidden });
    actions.push({ kind: "remove" });
    return actions;
  }

  if (!attacker || attacker.tokenId === target.tokenId) return [];
  return attacks.map((attack) => ({ kind: "attack", attack }));
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
    case "name":
      return action.hidden ? "Hide name from players" : "Show name to players";
    case "remove":
      return "Remove from the board…";
    case "place-token":
      return "Place a token here…";
    case "add-light":
      return "Add a light here";
  }
}

/** A stable test id per item. */
export function actionTestId(action: CanvasMenuAction): string {
  return action.kind === "attack"
    ? `canvas-menu-attack-${action.attack.abilityId}`
    : `canvas-menu-${action.kind}`;
}
