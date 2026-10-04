-- Play-view Combat: a persisted, shared initiative tracker.
--
-- Shared is the whole point: the GM and every player must see the same
-- turn order and the same active combatant. That rules out session-local
-- state, so turn order lives here and every change broadcasts on the
-- existing `world_events` bus (EVENT_CODE_COMBAT_CHANGED) — same
-- mechanism as walls/lights/tokens/Genie session state, no new transport.
--
-- One active combat per world at a time. Enforced by a partial unique
-- index rather than by convention, so a double-click on "Start combat"
-- cannot produce two live encounters that then disagree about whose turn
-- it is.
CREATE TABLE world_combats (
    id UUID PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    scene_id UUID NULL REFERENCES scenes(scene_id) ON DELETE SET NULL,
    -- 1-based, incremented when the turn pointer wraps past the last
    -- combatant.
    round INTEGER NOT NULL DEFAULT 1,
    -- The combatant whose turn it is. Nullable: a combat that has been
    -- started but has no combatants yet has no active turn.
    active_combatant_id UUID NULL,
    -- NULL while running; set when the GM ends the encounter. Ended
    -- combats are kept rather than deleted so the log stays honest.
    ended_at TIMESTAMP NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX world_combats_one_active_per_world_idx
    ON world_combats (world_id)
    WHERE ended_at IS NULL;

CREATE TABLE world_combatants (
    id UUID PRIMARY KEY,
    combat_id UUID NOT NULL REFERENCES world_combats(id) ON DELETE CASCADE,
    -- Both optional and independent: a combatant can be an actor with no
    -- token placed, a token with no actor row behind it (an ad-hoc
    -- monster), or a purely manual name the GM typed. `label` is what the
    -- tracker actually renders, so it is the only NOT NULL of the three.
    actor_id UUID NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    token_id UUID NULL,
    label TEXT NOT NULL,
    initiative INTEGER NOT NULL DEFAULT 0,
    -- Tiebreaker within an equal initiative, and the stable sort key that
    -- keeps ordering deterministic across clients. Without it two
    -- combatants on the same initiative could render in different orders
    -- for the GM and a player, which is exactly the disagreement this
    -- table exists to prevent.
    tiebreak INTEGER NOT NULL DEFAULT 0,
    is_npc BOOLEAN NOT NULL DEFAULT FALSE,
    -- Downed/removed combatants stay in the list, greyed out, rather than
    -- vanishing mid-encounter.
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX world_combatants_combat_idx ON world_combatants (combat_id);

-- Deferred to keep the two CREATE TABLEs independent of each other's
-- ordering: the active combatant must be one of this combat's own rows.
ALTER TABLE world_combats
    ADD CONSTRAINT world_combats_active_combatant_fkey
    FOREIGN KEY (active_combatant_id)
    REFERENCES world_combatants(id) ON DELETE SET NULL;
