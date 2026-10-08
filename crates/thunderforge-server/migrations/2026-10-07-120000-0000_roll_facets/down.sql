ALTER TABLE world_items DROP COLUMN IF EXISTS properties;

DROP INDEX IF EXISTS world_attacks_to_hit_roll_id_idx;
DROP INDEX IF EXISTS world_attacks_reroll_of_key;
ALTER TABLE world_attacks DROP COLUMN IF EXISTS reroll_of;

DROP INDEX IF EXISTS world_roll_records_reroll_of_key;
ALTER TABLE world_roll_records
    DROP CONSTRAINT IF EXISTS world_roll_records_reroll_pair,
    DROP CONSTRAINT IF EXISTS world_roll_records_check_id_for_checks,
    DROP COLUMN IF EXISTS reroll_spent,
    DROP COLUMN IF EXISTS reroll_of,
    DROP COLUMN IF EXISTS facets,
    DROP COLUMN IF EXISTS check_id,
    DROP COLUMN IF EXISTS roll_kind,
    DROP COLUMN IF EXISTS actor_id;
