-- Phase 4.9.B.1: Rollback players_online table

DROP TRIGGER IF EXISTS players_online_delete_notify_trigger ON players_online;
DROP FUNCTION IF EXISTS notify_players_online_delete();

DROP TRIGGER IF EXISTS players_online_notify_trigger ON players_online;
DROP FUNCTION IF EXISTS notify_players_online_change();

DROP TRIGGER IF EXISTS players_online_updated_at_trigger ON players_online;

DROP INDEX IF EXISTS idx_players_online_last_seen;
DROP INDEX IF EXISTS idx_players_online_player_id;
DROP INDEX IF EXISTS idx_players_online_world_id;

DROP TABLE IF EXISTS players_online;
