-- Spec 088 (FR-012): a world link's use limit is optional. NULL is no limit,
-- and no limit is the default. `used_count` still counts every join, limited
-- or not, and is only ever raised in the membership's own transaction.
--
-- The old default of 0 was one `CHECK (max_uses > 0)` refused, so the join
-- predicate's "0 means unlimited" branch could never match. Rows made before
-- this keep their limit and expiry.
ALTER TABLE world_invites DROP CONSTRAINT positive_max_uses;
ALTER TABLE world_invites DROP CONSTRAINT valid_used_count;
ALTER TABLE world_invites ALTER COLUMN max_uses DROP NOT NULL;
ALTER TABLE world_invites ALTER COLUMN max_uses SET DEFAULT NULL;
ALTER TABLE world_invites
    ADD CONSTRAINT positive_max_uses CHECK (max_uses IS NULL OR max_uses > 0),
    ADD CONSTRAINT valid_used_count
        CHECK (used_count >= 0 AND (max_uses IS NULL OR used_count <= max_uses));
