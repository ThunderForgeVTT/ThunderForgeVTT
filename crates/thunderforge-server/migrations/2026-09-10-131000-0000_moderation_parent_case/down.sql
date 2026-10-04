DROP INDEX IF EXISTS content_moderation_actions_parent_case_idx;
ALTER TABLE content_moderation_actions DROP COLUMN IF EXISTS parent_case_id;
