DROP INDEX IF EXISTS world_roll_records_world_created_idx;
ALTER TABLE world_roll_records
    DROP CONSTRAINT IF EXISTS world_roll_records_revealed_by_needs_revealed_at,
    DROP COLUMN revealed_by,
    DROP COLUMN revealed_at,
    DROP COLUMN label,
    DROP COLUMN visibility;
