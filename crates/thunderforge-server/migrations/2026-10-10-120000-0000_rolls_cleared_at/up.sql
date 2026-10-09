-- Spec 088 (FR-040): when the GM last cleared the roll feed. NULL is never.
-- A roll created at or before it is withheld from every path to a roll; the
-- rows themselves are kept, since attacks refer to them.
ALTER TABLE worlds ADD COLUMN rolls_cleared_at TIMESTAMPTZ NULL;
