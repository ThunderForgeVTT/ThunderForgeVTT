-- Rollback: Drop scenes table
DROP TRIGGER IF EXISTS update_scenes_updated_at ON scenes;
DROP TABLE IF EXISTS scenes CASCADE;
