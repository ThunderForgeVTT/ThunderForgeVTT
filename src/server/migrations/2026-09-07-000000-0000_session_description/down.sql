DROP INDEX IF EXISTS user_sessions_user_last_seen_idx;
ALTER TABLE user_sessions DROP CONSTRAINT IF EXISTS user_sessions_ended_reason_known;
ALTER TABLE user_sessions DROP COLUMN IF EXISTS ended_reason;
ALTER TABLE user_sessions DROP COLUMN IF EXISTS client_description;
ALTER TABLE user_sessions DROP COLUMN IF EXISTS last_seen_at;
