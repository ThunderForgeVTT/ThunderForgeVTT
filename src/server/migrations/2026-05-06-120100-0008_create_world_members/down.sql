-- Phase 4.10.A: Rollback world_members table

DROP TRIGGER IF EXISTS world_members_delete_notify_trigger ON world_members;
DROP FUNCTION IF EXISTS notify_world_members_delete();

DROP TRIGGER IF EXISTS world_members_notify_trigger ON world_members;
DROP FUNCTION IF EXISTS notify_world_members_change();

DROP TRIGGER IF EXISTS world_members_updated_at_trigger ON world_members;

DROP INDEX IF EXISTS idx_world_members_role;
DROP INDEX IF EXISTS idx_world_members_user_id;
DROP INDEX IF EXISTS idx_world_members_world_id;

DROP TABLE IF EXISTS world_members;
