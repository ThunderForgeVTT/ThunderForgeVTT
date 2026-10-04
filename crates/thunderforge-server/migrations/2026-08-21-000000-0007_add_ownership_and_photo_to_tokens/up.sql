-- Spec 004 / ADR-040: unify the token backing store onto this (scene-scoped)
-- tokens table. owner_user_id identifies the controlling player (their
-- primary token, or one the GM additionally granted); is_primary marks
-- exactly one token per (scene_id, owner_user_id) as that player's
-- default/profile token; photo_url is a player-editable avatar override
-- (falls back to the existing client-computed Dicebear URL when null);
-- health/max_health are ported from the now-retired world_tokens table.
ALTER TABLE tokens ADD COLUMN owner_user_id UUID NULL REFERENCES users(id);
ALTER TABLE tokens ADD COLUMN is_primary BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE tokens ADD COLUMN photo_url TEXT NULL;
ALTER TABLE tokens ADD COLUMN health INTEGER NULL;
ALTER TABLE tokens ADD COLUMN max_health INTEGER NULL;

CREATE UNIQUE INDEX tokens_one_primary_per_owner_per_scene
  ON tokens (scene_id, owner_user_id)
  WHERE is_primary;
