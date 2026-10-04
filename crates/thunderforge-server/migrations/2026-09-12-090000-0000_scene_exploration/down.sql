DROP TABLE IF EXISTS scene_exploration_resets;
ALTER TABLE scenes DROP COLUMN IF EXISTS exploration_epoch;
ALTER TABLE scenes DROP COLUMN IF EXISTS exploration_enabled;
