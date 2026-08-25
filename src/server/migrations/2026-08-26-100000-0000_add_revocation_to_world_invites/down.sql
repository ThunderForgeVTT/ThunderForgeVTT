-- Reverses 2026-08-26-100000-0000_add_revocation_to_world_invites.
--
-- Dropping `revoked` discards which links had been retired: on re-migration
-- every link reads active again. That is inherent to removing the column and
-- is why this is a down-migration rather than a routine operation.

DROP INDEX IF EXISTS world_invites_world_id_active_idx;

ALTER TABLE world_invites
    DROP COLUMN IF EXISTS rotated_from;

ALTER TABLE world_invites
    DROP COLUMN IF EXISTS revoked;
