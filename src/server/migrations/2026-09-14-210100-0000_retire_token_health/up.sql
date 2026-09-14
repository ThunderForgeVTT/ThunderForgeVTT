-- Spec 046 (FR-015; research R4; ADR-102): `tokens.health` and
-- `tokens.max_health` are retired.
--
-- They were a second record of a creature's hit points beside its actor's
-- system data, one no screen wrote. A creature has one record now: its actor's
-- (linked) or its own `system_data` (a copy). Dropping them is destructive, so
-- nothing is dropped until it has been put somewhere.
--
-- # Where a value goes
--
-- 1. **Every** non-null value is archived in `retired_token_health`, whatever
--    the token is, so the retirement loses nothing and `down.sql` restores
--    exactly what was there rather than reconstructing it.
-- 2. For an unlinked copy, the value is also written into `system_data`
--    through the hit-point fields its game system declares, so the copy's bars
--    and damage path see it.
--
-- # How the declared fields are resolved
--
-- A migration cannot read a pack's `system.json`. The fields below are the
-- ones `packs/systems/dnd5e/system.json` declares under `combat.hitPoints`
-- (`current_hp`, `max_hp`) when this was written, and 5e is the only bundled
-- pack that declares hit points at all. A token of any other system — or of an
-- installed pack this file cannot know about — is archived only: its value is
-- kept, not invented into fields its system never named. A linked token's
-- value is archived only as well: its actor is its record, and a second one
-- is the defect being removed.
--
-- The system is the actor's, else the world's, as `combat::hit_points` reads
-- it.

CREATE TABLE retired_token_health (
    token_id UUID PRIMARY KEY REFERENCES tokens(token_id) ON DELETE CASCADE,
    health INTEGER NULL,
    max_health INTEGER NULL,
    game_system_id VARCHAR NULL,
    -- True when the value was also written into the copy's `system_data`.
    copied_to_system_data BOOLEAN NOT NULL,
    -- No person retired these; the provenance is the scene's owner, as for
    -- the token itself.
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now()
);

INSERT INTO retired_token_health
    (token_id, health, max_health, game_system_id, copied_to_system_data,
     created_by, updated_by)
SELECT t.token_id,
       t.health,
       t.max_health,
       COALESCE(a.game_system_id, w.game_system_id),
       (t.linked = false AND COALESCE(a.game_system_id, w.game_system_id) = 'dnd5e'),
       s.owner_id,
       s.owner_id
  FROM tokens t
  JOIN scenes s ON s.scene_id = t.scene_id
  JOIN worlds w ON w.id = s.world_id
  LEFT JOIN world_actors a ON a.id = t.actor_id
 WHERE t.health IS NOT NULL OR t.max_health IS NOT NULL;

-- The token's own value is more specific than whatever the copy was seeded
-- with, so it wins field by field; a null is not written over a value.
UPDATE tokens t
   SET system_data = COALESCE(t.system_data, '{}'::jsonb)
       || jsonb_strip_nulls(jsonb_build_object(
              'current_hp', r.health,
              'max_hp', r.max_health))
  FROM retired_token_health r
 WHERE r.token_id = t.token_id
   AND r.copied_to_system_data;

DO $$
DECLARE
    archived INTEGER;
    copied INTEGER;
BEGIN
    SELECT count(*), count(*) FILTER (WHERE copied_to_system_data)
      INTO archived, copied
      FROM retired_token_health;
    RAISE NOTICE 'retire_token_health: archived % token(s), % also copied into system_data',
        archived, copied;
END $$;

ALTER TABLE tokens
    DROP COLUMN health,
    DROP COLUMN max_health;
