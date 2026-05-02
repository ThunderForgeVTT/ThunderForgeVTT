-- Remove schema_version from world_events
ALTER TABLE world_events DROP COLUMN IF EXISTS schema_version;
