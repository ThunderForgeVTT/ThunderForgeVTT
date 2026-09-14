-- Spec 046 (FR-015, FR-016, FR-017; research R4; ADR-102): a token is its
-- actor, or a copy of it.
--
-- A linked token has no data of its own: its hit points are its actor's
-- `world_actor_system_data.resource_data`. An unlinked copy holds
-- `system_data`, a `resource_data`-shaped object seeded from its actor when it
-- was placed, so two hundred goblins need no actors and a hit on one moves one
-- bar.

-- The column default is `true`: a token written by a path that does not
-- decide (bringing the party, older seeds, tests) reads its actor, which is
-- what every token did before this. `createToken` always decides explicitly.
ALTER TABLE tokens
    ADD COLUMN linked BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN system_data JSONB NULL;

ALTER TABLE world_actors
    ADD COLUMN is_unique BOOLEAN NOT NULL DEFAULT false;

-- Dangling actor ids first: the foreign key below cannot be added over them.
-- A token whose actor is gone was already a marker (nothing could read its
-- sheet); nulling the id says so. The count is reported, not guessed at.
DO $$
DECLARE
    dangling INTEGER;
BEGIN
    UPDATE tokens t
       SET actor_id = NULL
     WHERE t.actor_id IS NOT NULL
       AND NOT EXISTS (SELECT 1 FROM world_actors a WHERE a.id = t.actor_id);
    GET DIAGNOSTICS dangling = ROW_COUNT;
    RAISE NOTICE 'token_links: nulled % dangling tokens.actor_id value(s)', dangling;
END $$;

-- Backfill by what the actor is, not by `token_type`: NPC tokens have been
-- written as `character` (research R4, `parse_token_kind(None)`), so the type
-- is not evidence of anything.
--
-- - a player character's token stays linked;
-- - an NPC's token becomes a copy, seeded from that NPC's data as it stands
--   now, so its bars do not vanish and it stops sharing a pool with every
--   other token of the same NPC;
-- - a token with no actor is a marker: unlinked, no data.
UPDATE tokens t
   SET linked = false,
       system_data = (
           SELECT d.resource_data
             FROM world_actor_system_data d
            WHERE d.actor_id = a.id
       )
  FROM world_actors a
 WHERE a.id = t.actor_id
   AND a.is_npc = true;

UPDATE tokens
   SET linked = false
 WHERE actor_id IS NULL;

ALTER TABLE tokens
    ADD CONSTRAINT tokens_linked_has_no_system_data
        CHECK (linked = false OR system_data IS NULL);

ALTER TABLE tokens
    ADD CONSTRAINT tokens_actor_id_fkey
        FOREIGN KEY (actor_id) REFERENCES world_actors(id) ON DELETE SET NULL;
