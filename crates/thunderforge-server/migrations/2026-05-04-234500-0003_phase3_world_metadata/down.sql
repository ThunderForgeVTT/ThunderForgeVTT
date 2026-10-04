DROP INDEX IF EXISTS idx_policies_world_id;

ALTER TABLE policies
    DROP CONSTRAINT IF EXISTS policies_world_id_fkey,
    DROP COLUMN IF EXISTS world_id;

DROP INDEX IF EXISTS idx_worlds_owner_name_unique;

ALTER TABLE worlds
    DROP COLUMN IF EXISTS interface_pack_id,
    DROP COLUMN IF EXISTS game_system_id,
    DROP COLUMN IF EXISTS description;
