-- Spec 027 (T002, FR-001/FR-002/FR-003/FR-007): world_invites becomes a
-- revocable, rotatable access link.
--
-- Deliberately ADDITIVE. FR-007 requires every code already in circulation to
-- keep working, so no existing column changes type or meaning and no row is
-- rewritten. DEFAULT FALSE is what makes every pre-existing invite read as
-- active rather than revoked — that default is load-bearing, not cosmetic.
--
-- invite_code is already VARCHAR(32), so the move from 8- to 20-character
-- codes needs no width change here.

ALTER TABLE world_invites
    ADD COLUMN revoked BOOLEAN NOT NULL DEFAULT FALSE;

-- Lineage for a rotated link: set on the replacement, NULL on an original.
-- Self-referential and nullable. ON DELETE SET NULL rather than CASCADE —
-- deleting a retired link must never take its successor with it.
ALTER TABLE world_invites
    ADD COLUMN rotated_from UUID NULL REFERENCES world_invites(id) ON DELETE SET NULL;

-- The GM panel lists a world's usable links; this keeps that lookup cheap
-- without touching the existing unique index on invite_code, which is the
-- collision guard the spec-005 fix depends on.
CREATE INDEX world_invites_world_id_active_idx
    ON world_invites (world_id)
    WHERE revoked = FALSE;
