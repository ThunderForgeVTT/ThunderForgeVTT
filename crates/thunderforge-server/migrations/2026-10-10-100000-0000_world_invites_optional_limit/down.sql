-- A link with no limit gets one it has not reached.
UPDATE world_invites SET max_uses = GREATEST(used_count, 1) + 50 WHERE max_uses IS NULL;
ALTER TABLE world_invites DROP CONSTRAINT positive_max_uses;
ALTER TABLE world_invites DROP CONSTRAINT valid_used_count;
ALTER TABLE world_invites ALTER COLUMN max_uses SET NOT NULL;
ALTER TABLE world_invites ALTER COLUMN max_uses SET DEFAULT 0;
ALTER TABLE world_invites
    ADD CONSTRAINT positive_max_uses CHECK (max_uses > 0),
    ADD CONSTRAINT valid_used_count CHECK (used_count >= 0 AND used_count <= max_uses);
