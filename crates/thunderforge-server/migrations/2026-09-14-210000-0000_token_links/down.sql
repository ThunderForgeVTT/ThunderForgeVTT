-- Reverses 2026-09-14-210000-0000_token_links.
--
-- What it cannot reverse: dangling `actor_id` values nulled on the way up are
-- not restored (the actors they named do not exist), and a copy's own hit
-- points go with `system_data` — every token reads its actor again, which is
-- how tokens behaved before.
ALTER TABLE tokens DROP CONSTRAINT IF EXISTS tokens_actor_id_fkey;
ALTER TABLE tokens DROP CONSTRAINT IF EXISTS tokens_linked_has_no_system_data;
ALTER TABLE tokens DROP COLUMN IF EXISTS system_data;
ALTER TABLE tokens DROP COLUMN IF EXISTS linked;
ALTER TABLE world_actors DROP COLUMN IF EXISTS is_unique;
