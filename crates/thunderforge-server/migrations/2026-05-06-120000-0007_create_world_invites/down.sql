-- Phase 4.10.A: Rollback world_invites table

DROP TRIGGER IF EXISTS world_invites_notify_trigger ON world_invites;
DROP FUNCTION IF EXISTS notify_world_invites_change();

DROP TRIGGER IF EXISTS world_invites_updated_at_trigger ON world_invites;

DROP INDEX IF EXISTS idx_world_invites_expires_at;
DROP INDEX IF EXISTS idx_world_invites_invite_code;
DROP INDEX IF EXISTS idx_world_invites_created_by;
DROP INDEX IF EXISTS idx_world_invites_world_id;

DROP TABLE IF EXISTS world_invites;
