-- Spec 084: what a roll was for, the facets that shaped it, and the roll it
-- replaces when a resource was spent to reroll it.
--
-- Every existing row reads as before: no actor, no kind, no facets, not a
-- reroll, and every item without properties.
ALTER TABLE world_roll_records
    ADD COLUMN actor_id UUID NULL REFERENCES world_actors(id) ON DELETE SET NULL,
    ADD COLUMN roll_kind TEXT NULL
        CHECK (roll_kind IS NULL OR roll_kind IN ('check', 'to_hit', 'damage')),
    ADD COLUMN check_id TEXT NULL,
    ADD COLUMN facets TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN reroll_of UUID NULL REFERENCES world_roll_records(id) ON DELETE CASCADE,
    ADD COLUMN reroll_spent TEXT NULL,
    ADD CONSTRAINT world_roll_records_check_id_for_checks
        CHECK ((roll_kind = 'check') = (check_id IS NOT NULL)),
    ADD CONSTRAINT world_roll_records_reroll_pair
        CHECK ((reroll_of IS NULL) = (reroll_spent IS NULL));

-- One replacement per roll, whatever was spent (research R8).
CREATE UNIQUE INDEX world_roll_records_reroll_of_key
    ON world_roll_records (reroll_of) WHERE reroll_of IS NOT NULL;

ALTER TABLE world_attacks
    ADD COLUMN reroll_of UUID NULL REFERENCES world_attacks(id) ON DELETE CASCADE;
CREATE UNIQUE INDEX world_attacks_reroll_of_key
    ON world_attacks (reroll_of) WHERE reroll_of IS NOT NULL;
-- A reroll looks its attack up by the roll.
CREATE INDEX IF NOT EXISTS world_attacks_to_hit_roll_id_idx
    ON world_attacks (to_hit_roll_id);

ALTER TABLE world_items
    ADD COLUMN properties TEXT[] NOT NULL DEFAULT '{}';
