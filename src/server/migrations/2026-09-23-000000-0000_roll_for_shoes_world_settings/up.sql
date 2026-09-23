-- Spec 062: the five optional rules Roll for Shoes leaves to the table, as
-- per-world settings (data-model.md §1).
--
-- A table the pack owns rather than five columns on `worlds` (ADR-063,
-- research D1). `worlds.genie_resource_carryover_enabled` is the shape this
-- avoids: a column named for one ruleset on a table every system shares, which
-- `check-system-registry.mjs` cannot catch because a column name is neither a
-- quoted id nor a filename. Five of those would make one mistake five.
--
-- There is no row for a world until a Game Master changes something. A missing
-- row reads as every default, so a world that predates this migration and a
-- world whose settings have never been touched are indistinguishable — which
-- is what makes the whole feature opt-in (FR-001).

CREATE TABLE world_roll_for_shoes_settings (
    world_id UUID PRIMARY KEY REFERENCES worlds(id) ON DELETE CASCADE,

    -- How the Game Master states the opposition: a free number as the core
    -- game has always allowed, a pool of GM dice by band, or a static target
    -- by band. The free number stays available in every mode (FR-011).
    difficulty_mode TEXT NOT NULL DEFAULT 'free'
        CHECK (difficulty_mode IN ('free', 'rolled', 'target')),

    -- On, a tie is a success and therefore awards no XP. Off is the core
    -- rule: "higher than", so a tie is a failure and earns 1 XP.
    tie_succeeds BOOLEAN NOT NULL DEFAULT FALSE,

    -- On, characters may carry named conditions whose signed modifiers sum
    -- and apply flat to a roll's total — never to the dice.
    statuses_enabled BOOLEAN NOT NULL DEFAULT FALSE,

    -- On, per-level caps apply to gaining a skill. The cap governs a
    -- transition, not a stored shape: a character that already exceeds it
    -- stays storable (FR-036).
    skill_slots_enabled BOOLEAN NOT NULL DEFAULT FALSE,

    -- `[{ "name": ..., "level": ... }]`. Empty means the core default,
    -- "Do Anything 1" — not "a character starts with no skills".
    starting_skills JSONB NOT NULL DEFAULT '[]'::jsonb,

    updated_by UUID REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
