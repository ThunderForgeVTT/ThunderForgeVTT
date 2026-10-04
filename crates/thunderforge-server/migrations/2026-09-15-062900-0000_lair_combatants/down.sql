ALTER TABLE world_attacks DROP COLUMN IF EXISTS attacker_kind;
ALTER TABLE world_combatants
    DROP CONSTRAINT IF EXISTS world_combatants_lair_has_no_creature,
    DROP COLUMN IF EXISTS kind;
