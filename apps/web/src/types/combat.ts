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
  activeCombatantId: string | null;
  /** Set once the GM ends the encounter. */
  endedAt: string | null;
  /** Already in turn order — render as given, never re-sort. */
  combatants: CombatantRecord[];
}
