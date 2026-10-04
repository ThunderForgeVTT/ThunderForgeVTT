-- A player's activation awaiting a Game Master's decision.
--
-- Session-scoped and pruned rather than retained. In-memory would be lighter,
-- but a GM who refreshes mid-session must not lose a pending request, and a GM
-- on a second device must see the same queue. Presence went to memory earlier
-- in this project for a load reason that does not apply here: these are a
-- handful per session, not a heartbeat per client.
--
-- Deliberately not an audit log. Retaining who asked to go where is a privacy
-- surface with no stated purpose.
CREATE TABLE interaction_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    interactive_id UUID NOT NULL REFERENCES interactives(interactive_id) ON DELETE CASCADE,
    scene_id UUID NOT NULL REFERENCES scenes(scene_id) ON DELETE CASCADE,
    requested_by UUID NOT NULL REFERENCES users(id),

    -- 'pending' | 'approved' | 'refused' | 'cancelled'
    --
    -- Nothing moves this to 'approved' except a Game Master. There is no
    -- expiry, no timeout, no default: silence is not consent.
    state VARCHAR NOT NULL DEFAULT 'pending',

    decided_by UUID REFERENCES users(id),
    decided_at TIMESTAMP,

    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now()
);

-- The GM's queue is "what is pending in this scene".
CREATE INDEX interaction_requests_scene_state_idx
    ON interaction_requests (scene_id, state);

-- A requester leaving cancels their pending requests.
CREATE INDEX interaction_requests_requester_idx
    ON interaction_requests (requested_by);
