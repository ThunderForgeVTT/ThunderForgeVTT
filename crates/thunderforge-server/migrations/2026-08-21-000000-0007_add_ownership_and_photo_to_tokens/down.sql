DROP INDEX IF EXISTS tokens_one_primary_per_owner_per_scene;
ALTER TABLE tokens DROP COLUMN IF EXISTS max_health;
ALTER TABLE tokens DROP COLUMN IF EXISTS health;
ALTER TABLE tokens DROP COLUMN IF EXISTS photo_url;
ALTER TABLE tokens DROP COLUMN IF EXISTS is_primary;
ALTER TABLE tokens DROP COLUMN IF EXISTS owner_user_id;
