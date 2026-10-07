-- Spec 081: who may see a roll, what it was for, and whether the GM has
-- since shown it to the table.
--
-- Every roll before this column was seen by no one but the GM's history, so
-- `everyone` is the honest default for them: nothing hidden is made public.
ALTER TABLE world_roll_records
    ADD COLUMN visibility TEXT NOT NULL DEFAULT 'everyone'
        CHECK (visibility IN ('everyone', 'gm_eyes', 'gm_only')),
    ADD COLUMN label TEXT CHECK (char_length(label) <= 80),
    ADD COLUMN revealed_at TIMESTAMPTZ,
    ADD COLUMN revealed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD CONSTRAINT world_roll_records_revealed_by_needs_revealed_at
        CHECK (revealed_at IS NOT NULL OR revealed_by IS NULL);

-- The feed pages one world's rolls newest first.
CREATE INDEX world_roll_records_world_created_idx
    ON world_roll_records (world_id, created_at DESC);
