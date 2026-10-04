-- Spec 034: the short-lived record of a grant a Game Master started.
--
-- Modelled on `oauth_authorization_sessions`, and for the same reason: a
-- hand-off leaves this application and comes back, and the thing that comes
-- back has to be tied to the thing that left. Without that, anyone who can
-- reach the callback can attach an installation of their own choosing to a
-- world they do not own.
--
-- What it binds together: the world being connected, the person who started
-- it, and an unguessable value echoed by the host. All three are checked on
-- return, because two of them are not enough — a valid state for the wrong
-- world, or the right world claimed by the wrong person, are both attacks.
CREATE TABLE lore_sync_grant_sessions (
    id UUID PRIMARY KEY,

    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,

    -- Who started it. Re-checked on return rather than trusted, because
    -- authority over a world can change between leaving and coming back
    -- (FR-003), and a hand-off is exactly the kind of pause where it does.
    started_by UUID NOT NULL REFERENCES users(id),

    -- The anti-forgery value. v4-derived and unguessable, never v7: a v7
    -- front-loads a timestamp, which narrows the search space and leaks when
    -- the flow began. ADR-049 makes the same call for share codes.
    state VARCHAR NOT NULL UNIQUE,

    -- Where in the app to return the user afterwards.
    return_to VARCHAR,

    -- Short-lived on purpose. A grant hand-off is a few seconds of human
    -- attention; anything still open an hour later is abandoned, and an
    -- abandoned session that stays valid is an attack surface that grows.
    expires_at TIMESTAMP NOT NULL,

    -- Single-use. Without this, a captured callback URL could be replayed to
    -- rebind a world after the fact.
    consumed_at TIMESTAMP,

    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX lore_sync_grant_sessions_state ON lore_sync_grant_sessions (state);
