-- Spec 020 (FR-003, research.md R1): per-world GM setting controlling
-- whether Genie Session Resource holdings carry over into the next
-- session's holdings ("the rope doesn't disappear") or reset to 0
-- (default, matching the Wish Pool's existing per-session reset).
ALTER TABLE worlds
    ADD COLUMN genie_resource_carryover_enabled BOOLEAN NOT NULL DEFAULT false;
