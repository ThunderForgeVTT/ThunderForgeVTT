-- Add schema_version tracking to world_events for future migrations
ALTER TABLE world_events ADD COLUMN schema_version INTEGER NOT NULL DEFAULT 1;
