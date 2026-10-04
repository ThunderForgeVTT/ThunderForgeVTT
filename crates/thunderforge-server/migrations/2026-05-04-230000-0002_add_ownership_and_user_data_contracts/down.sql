ALTER TABLE world_events
    DROP CONSTRAINT IF EXISTS world_events_world_id_fkey;

ALTER TABLE world_events
    ADD CONSTRAINT world_events_world_id_fkey
    FOREIGN KEY (world_id) REFERENCES worlds (id);

DROP INDEX IF EXISTS idx_policies_created_by;
DROP INDEX IF EXISTS idx_world_events_created_by;
DROP INDEX IF EXISTS idx_world_tokens_created_by;
DROP INDEX IF EXISTS idx_worlds_created_by;

ALTER TABLE policies
    DROP CONSTRAINT IF EXISTS policies_updated_by_fkey,
    DROP CONSTRAINT IF EXISTS policies_created_by_fkey,
    DROP COLUMN IF EXISTS updated_by,
    DROP COLUMN IF EXISTS created_by;

ALTER TABLE world_events
    DROP CONSTRAINT IF EXISTS world_events_updated_by_fkey,
    DROP CONSTRAINT IF EXISTS world_events_created_by_fkey,
    DROP COLUMN IF EXISTS updated_at,
    DROP COLUMN IF EXISTS updated_by,
    DROP COLUMN IF EXISTS created_by;

ALTER TABLE world_tokens
    DROP CONSTRAINT IF EXISTS world_tokens_updated_by_fkey,
    DROP CONSTRAINT IF EXISTS world_tokens_created_by_fkey,
    DROP COLUMN IF EXISTS updated_by,
    DROP COLUMN IF EXISTS created_by;

ALTER TABLE worlds
    DROP CONSTRAINT IF EXISTS worlds_updated_by_fkey,
    DROP CONSTRAINT IF EXISTS worlds_created_by_fkey,
    DROP COLUMN IF EXISTS updated_by,
    DROP COLUMN IF EXISTS created_by;
