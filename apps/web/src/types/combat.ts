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
  /** Already in turn order — render as given, never re-sort. */
  combatants: CombatantRecord[];
}
