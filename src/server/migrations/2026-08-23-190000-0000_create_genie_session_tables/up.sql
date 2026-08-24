-- Spec 018 (User Story 7): Session Wish Pool, Doom Clock, Puzzle Clocks,
-- and Session Resource holdings — genuinely new session-scoped shared
-- party state (data-model.md "Session Wish Pool + Doom Clock",
-- "world_genie_puzzle_clocks", "world_genie_resource_holdings").
--
-- A `world_genie_trade_proposals` table is also created here to back the
-- two-party-consent `proposeResourceTrade`/`acceptResourceTrade` flow
-- (research.md R8, contracts/genie-session-loop.md) — a small, persisted
-- table rather than pure in-memory state, so a pending proposal survives
-- a server restart / scales across instances, consistent with how the
-- rest of this server persists state.

CREATE TABLE world_genie_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    wishes_remaining INTEGER NOT NULL DEFAULT 3 CHECK (wishes_remaining >= 0),
    doom_clock_current INTEGER NOT NULL DEFAULT 0 CHECK (doom_clock_current >= 0),
    doom_clock_max INTEGER NOT NULL CHECK (doom_clock_max > 0),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'won', 'lost')),
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (doom_clock_current <= doom_clock_max)
);

CREATE INDEX world_genie_sessions_world_id_idx ON world_genie_sessions(world_id);

CREATE TABLE world_genie_puzzle_clocks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES world_genie_sessions(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    segments_current INTEGER NOT NULL DEFAULT 0 CHECK (segments_current >= 0),
    segments_max INTEGER NOT NULL CHECK (segments_max > 0),
    resolved_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (segments_current <= segments_max)
);

CREATE INDEX world_genie_puzzle_clocks_session_id_idx ON world_genie_puzzle_clocks(session_id);

CREATE TABLE world_genie_resource_holdings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES world_genie_sessions(id) ON DELETE CASCADE,
    actor_id UUID NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    resource_type TEXT NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 0 CHECK (quantity >= 0),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (session_id, actor_id, resource_type)
);

CREATE INDEX world_genie_resource_holdings_session_id_idx ON world_genie_resource_holdings(session_id);
CREATE INDEX world_genie_resource_holdings_actor_id_idx ON world_genie_resource_holdings(actor_id);

CREATE TABLE world_genie_trade_proposals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES world_genie_sessions(id) ON DELETE CASCADE,
    from_actor_id UUID NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    from_resource_type TEXT NOT NULL,
    from_quantity INTEGER NOT NULL CHECK (from_quantity > 0),
    to_actor_id UUID NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    to_resource_type TEXT NOT NULL,
    to_quantity INTEGER NOT NULL CHECK (to_quantity > 0),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected', 'expired')),
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (from_actor_id != to_actor_id)
);

CREATE INDEX world_genie_trade_proposals_session_id_idx ON world_genie_trade_proposals(session_id);
CREATE INDEX world_genie_trade_proposals_to_actor_id_idx ON world_genie_trade_proposals(to_actor_id);
