DROP TABLE IF EXISTS world_offers;
DROP TABLE IF EXISTS world_attacks;

ALTER TABLE world_items
    DROP CONSTRAINT IF EXISTS world_items_range_long_not_short,
    DROP COLUMN IF EXISTS multiattack,
    DROP COLUMN IF EXISTS legendary_cost,
    DROP COLUMN IF EXISTS action_cost,
    DROP COLUMN IF EXISTS needs_line_of_sight,
    DROP COLUMN IF EXISTS range_long,
    DROP COLUMN IF EXISTS range_normal,
    DROP COLUMN IF EXISTS reach;

ALTER TABLE world_abilities
    DROP CONSTRAINT IF EXISTS world_abilities_range_long_not_short,
    DROP COLUMN IF EXISTS multiattack,
    DROP COLUMN IF EXISTS legendary_cost,
    DROP COLUMN IF EXISTS action_cost,
    DROP COLUMN IF EXISTS needs_line_of_sight,
    DROP COLUMN IF EXISTS range_long,
    DROP COLUMN IF EXISTS range_normal,
    DROP COLUMN IF EXISTS reach;

ALTER TABLE world_combats DROP COLUMN IF EXISTS auto_apply;
ALTER TABLE worlds DROP COLUMN IF EXISTS auto_apply_npc_damage;
