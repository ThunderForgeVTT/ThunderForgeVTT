-- Exactly what up.sql created, in reverse. The record goes with it; that is
-- what a downgrade past this feature means.
DROP TRIGGER IF EXISTS world_play_pause_triggers_append_only_trigger ON world_play_pause_triggers;
DROP FUNCTION IF EXISTS world_play_pause_triggers_append_only();
DROP TRIGGER IF EXISTS world_play_pause_requests_decided_is_final_trigger ON world_play_pause_requests;
DROP FUNCTION IF EXISTS world_play_pause_requests_decided_is_final();
DROP TRIGGER IF EXISTS world_play_pauses_only_lift_trigger ON world_play_pauses;
DROP FUNCTION IF EXISTS world_play_pauses_only_lift();

DROP TABLE IF EXISTS world_live_play;
DROP TABLE IF EXISTS world_play_pause_triggers;
DROP TABLE IF EXISTS world_play_pauses;
DROP TABLE IF EXISTS world_play_pause_requests;

DROP TYPE IF EXISTS "PauseTriggerKind";
DROP TYPE IF EXISTS "PauseRequestState";
