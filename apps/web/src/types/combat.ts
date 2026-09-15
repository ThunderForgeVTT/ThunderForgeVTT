/** One entry in the initiative order (`world_combatants`). */
export interface CombatantRecord {
  id: string;
  combatId: string;
  /** Set when this combatant is backed by a world actor. */
  actorId: string | null;
  /** Set when this combatant is backed by a placed token. */
  tokenId: string | null;
  label: string;
  initiative: number;
  /** Tiebreaker within an equal initiative. */
  tiebreak: number;
  isNpc: boolean;
  /** False for downed/removed combatants, which stay in the list greyed out. */
  active: boolean;
  /**
   * Spec 046: why an inactive combatant is out. `HIT_POINTS` when its
   * creature reached zero (healing brings it back), `GAME_MASTER` when the
   * tracker's Down was pressed (healing does not), null when in the fight.
   */
  downedBy: CombatantDownedBy | null;
  /**
   * Spec 046 US5: what this combatant's turn affords and what it has spent,
   * shown to every seat. Null when the system declares no turn budget.
   */
  budget: TurnBudgetRecord | null;
}

/**
 * One line of a turn's budget. `remaining` may be negative: an overspend is
 * recorded and shown, never refused (spec 046 C9), and a table may treat it
 * as a debt.
 */
export interface BudgetLineRecord {
  allowed: number;
  spent: number;
  remaining: number;
}

/** `TurnBudget`: a combatant's action economy for its current turn. */
export interface TurnBudgetRecord {
  action: BudgetLineRecord;
  bonusAction: BudgetLineRecord;
  reaction: BudgetLineRecord;
  /** In the system's units (`unit`). */
  movement: BudgetLineRecord;
  /** Null when the creature has no legendary actions. */
  legendary: BudgetLineRecord | null;
  /** The system's unit for distances, e.g. "ft". */
  unit: string;
}

export type CombatantDownedBy = "HIT_POINTS" | "GAME_MASTER";

/** Which way `changeHitPoints` moves a creature's hit points. */
export type HitPointChange = "DAMAGE" | "HEALING";

/** A creature's hit points after a change (`TokenHitPoints`). */
export interface TokenHitPointsRecord {
  tokenId: string;
  current: number;
  max: number;
  temporary: number;
}

/** A shared, persisted encounter (`world_combats`). */
export interface CombatRecord {
  id: string;
  worldId: string;
  sceneId: string | null;
  round: number;
  /**
   * What this ruleset calls a round, or null when it does not count them.
   *
   * Spec 031 FR-031/SC-011: turn structure is the system's to determine.
   * Absent means show no counter at all — Blades in the Dark has no turn
   * order, and a "Round 1" over a game that has no rounds is the product
   * asserting a rule the ruleset does not have.
   */
  roundLabel: string | null;
  activeCombatantId: string | null;
  /** Set once the GM ends the encounter. */
  endedAt: string | null;
  /**
   * Spec 046 FR-006: this encounter's auto-apply override; null uses the
   * world's setting. Ends with the encounter.
   */
  autoApply: boolean | null;
  /** Whether hits on the Game Master's NPCs are applied in this encounter. */
  effectiveAutoApply: boolean;
  /** Already in turn order — render as given, never re-sort. */
  combatants: CombatantRecord[];
}
