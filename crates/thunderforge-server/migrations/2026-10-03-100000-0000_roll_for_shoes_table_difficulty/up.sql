-- What the Game Master has said has to be beaten, held for the whole table.
--
-- Roll for Shoes is played mostly without a map: the Game Master calls how
-- hard something is and everyone rolls against that. Until now the number
-- lived in whichever browser typed it — the player's, usually — and in the
-- "rolled" difficulty mode the player rolled the Game Master's dice and could
-- roll them again. This is the number said once, by the Game Master, where
-- every sheet at the table reads it.
--
-- A table the pack owns, per ADR-063, beside `world_roll_for_shoes_settings`
-- and for the same reason: it is one ruleset's state and belongs on no table
-- every system shares.
--
-- A row means a difficulty is set. No row means none is, and the sheet falls
-- back to the free entry it has always had — so a table with no Game Master
-- at the keyboard plays exactly as before.

CREATE TABLE world_roll_for_shoes_difficulty (
    world_id UUID PRIMARY KEY REFERENCES worlds(id) ON DELETE CASCADE,

    -- The number a roll's total is compared with. Always present: a band is a
    -- way of arriving at this number, never a second kind of opposition.
    target INTEGER NOT NULL,

    -- How hard the Game Master called it, when they called it by name rather
    -- than by number. Spelled as the pack's web half spells it.
    band TEXT
        CHECK (band IS NULL OR band IN ('easy', 'moderate', 'hard', 'veryHard')),

    -- The Game Master's dice, when this world rolls for difficulty: the faces
    -- the server rolled, once, whose sum is `target`. NULL when the number was
    -- named or fixed by its band.
    gm_dice JSONB,

    set_by UUID REFERENCES users(id),
    set_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
