/**
 * worldActorSystemDataCollection.ts
 * Plain types/helpers for actor system-specific data (ability/resource/
 * proficiency/trait/spell_data JSONB columns).
 *
 * RxDB removal (hard cut, unreleased project — see useActorSystemData.ts /
 * useUpdateActorData.ts): this used to be an RxDB collection schema plus a
 * replication setup function. Neither the schema nor the replication path
 * were ever wired into a running database (see database.ts's history) or
 * consumed by anything other than each other, so they were deleted rather
 * than fixed. `WorldActorSystemDataDoc` and `computeActorDerivedStats` are
 * kept because real callers still use them as a plain data shape.
 *
 * Example: D&D 5e character stores all stats as JSONB in one row per actor:
 *   ability_data: { "strength": 10, "dexterity": 12, ... }
 *   resource_data: { "current_hp": 45, "max_hp": 50, ... }
 *   proficiency_data: { "proficient_in_acrobatics": true, ... }
 *   trait_data: { "class": "rogue", "level": 5, ... }
 *   spell_data: { "spellcasting_ability": "dexterity", "spell_slots": {...} }
 *
 * Pathfinder 2e character stores DIFFERENT structure in SAME columns:
 *   ability_data: { "strength_mod": 0, "reflex_mod": 2, ... }
 *   etc.
 */

export interface WorldActorSystemDataDoc {
  id: string;
  actor_id: string;
  game_system_id: string;

  ability_data?: Record<string, any>;
  resource_data?: Record<string, any>;
  proficiency_data?: Record<string, any>;
  trait_data?: Record<string, any>;
  spell_data?: Record<string, any>;

  created_by: string;
  updated_by: string;
  created_at: string;
  updated_at: string;

  _optimistic?: boolean;
  _lastServerData?: {
    ability_data?: Record<string, any>;
    resource_data?: Record<string, any>;
    proficiency_data?: Record<string, any>;
    trait_data?: Record<string, any>;
    spell_data?: Record<string, any>;
  };
}

/**
 * Compute derived stats from base actor system data.
 *
 * Called locally after data updates to avoid sending derived data over network.
 * Example: health_percentage, ability_modifiers, skill_bonuses, etc.
 *
 * System-specific derivation happens in game-system plugins (Phase E).
 */
export function computeActorDerivedStats(
  data: WorldActorSystemDataDoc,
  // Kept in the signature because every caller has one and a future rule
  // will be system-specific; nothing here branches on it yet.
  _gameSystemId: string,
) {
  const hasAbilityData =
    !!data.ability_data && Object.keys(data.ability_data).length > 0;
  const hasResourceData =
    !!data.resource_data && Object.keys(data.resource_data).length > 0;
  const hasTraitData =
    !!data.trait_data && Object.keys(data.trait_data).length > 0;

  const baseStats = {
    isFullyConfigured: hasAbilityData && hasResourceData && hasTraitData,
    lastUpdated: new Date(data.updated_at).getTime(),
    age: Date.now() - new Date(data.updated_at).getTime(),
  };

  // System-specific derivation (to be implemented in Phase E game system plugins)
  // For now, just return base stats
  return baseStats;
}
