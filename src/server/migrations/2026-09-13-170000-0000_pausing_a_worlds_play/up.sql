-- Spec 051 (FR-001 to FR-053), ADR-100: an operator can pause a world's play.
--
-- Three record tables, one fact table and two enums. Three things are worth
-- saying plainly, because each is a decision the feature cannot take back
-- cheaply:
--
-- **The record has no foreign key to `worlds` or `users`** (research R8). A
-- pause, a request and what led to either store ids beside name snapshots, the
-- way `content_moderation_actions` does, so the record outlives the world it
-- stopped (FR-053) and the account of the operator who stopped it. A `SET
-- NULL` audit column would keep the row and lose its *who*, and a record of
-- who paused a table must not lose that.
--
-- **The record is append-only, and the database says so.** A pause may be
-- lifted, once, and nothing else about it changes. A request may be decided,
-- once, and nothing else about it changes. None of the three may be deleted.
-- These are triggers rather than a rule in the module because the module is
-- not the only thing that can reach Postgres, and a record an operator can
-- quietly rewrite is not a record.
--
-- **`world_live_play` is the exception to all of that.** It is not a record,
-- it is a fact about a world that exists now, and it goes with the world.

-- Where a request stands (FR-032 to FR-035).
--
-- PascalCase follows "ContentOrigin" and "DeltaForm".
CREATE TYPE "PauseRequestState" AS ENUM ('Pending', 'Approved', 'Declined');

-- What led to a request or a pause (FR-037).
--
-- Open on purpose: nothing raises `AbuseReport` yet, and spec 042's queue or an
-- abuse intake can raise requests later without changing the request's shape.
CREATE TYPE "PauseTriggerKind" AS ENUM ('Takedown', 'Operator', 'AbuseReport');

-- A proposal to pause a world, waiting for an operator.
--
-- Created before `world_play_pauses` because an approved pause names the
-- request it came from.
CREATE TABLE world_play_pause_requests (
    id UUID PRIMARY KEY,

    -- No foreign key (research R8). The name is the world's at the moment the
    -- request was raised, so an operator reading a request about a world since
    -- deleted still knows which table it was.
    world_id UUID NOT NULL,
    world_name TEXT NOT NULL,

    raised_at TIMESTAMP NOT NULL DEFAULT now(),
    state "PauseRequestState" NOT NULL DEFAULT 'Pending',

    decided_by UUID,
    decided_by_name TEXT,
    decided_at TIMESTAMP,
    -- Required with a decision. On approval it becomes the pause's grounds.
    decision_note TEXT,

    -- NULL when the system raised it: a takedown filed by an anonymous
    -- claimant has nobody to name.
    created_by UUID,
    updated_by UUID,
    updated_at TIMESTAMP NOT NULL DEFAULT now(),

    -- Decided means fully decided. A request with a decision and no decider,
    -- or a decider and no note, is one nobody could audit.
    CONSTRAINT world_play_pause_requests_decided_in_full CHECK (
        (state = 'Pending') = (decided_at IS NULL)
        AND (state = 'Pending') = (decided_by IS NULL)
        AND (state = 'Pending') = (decided_by_name IS NULL)
        AND (state = 'Pending') = (decision_note IS NULL)
    ),

    CONSTRAINT world_play_pause_requests_note_is_not_blank CHECK (
        decision_note IS NULL OR length(btrim(decision_note)) > 0
    )
);

-- One pending request per world (FR-033). A second takedown on a world already
-- waiting for an operator adds a trigger to the request that exists; it does
-- not make an operator decide the same world twice.
CREATE UNIQUE INDEX world_play_pause_requests_one_pending
    ON world_play_pause_requests (world_id) WHERE state = 'Pending';

-- A world's live play stopped by an operator. While `lifted_at` is NULL, the
-- world is paused.
CREATE TABLE world_play_pauses (
    id UUID PRIMARY KEY,

    world_id UUID NOT NULL,
    world_name TEXT NOT NULL,

    paused_by UUID NOT NULL,
    paused_by_name TEXT NOT NULL,
    paused_at TIMESTAMP NOT NULL DEFAULT now(),

    -- Why (FR-004). Never shown to a member of the world (FR-011).
    grounds TEXT NOT NULL,

    -- The approved request, when the pause came from one. Its triggers stay on
    -- the request and are reached through this column.
    request_id UUID REFERENCES world_play_pause_requests(id),

    lifted_by UUID,
    lifted_by_name TEXT,
    lifted_at TIMESTAMP,
    lift_grounds TEXT,

    -- Principle III provenance, beside the semantic columns above.
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT now(),

    CONSTRAINT world_play_pauses_grounds_are_not_blank CHECK (
        length(btrim(grounds)) > 0
    ),

    -- Lifting is all-or-nothing. `lift_grounds` is held to the same rule as
    -- the other three: a lift is refused on blank grounds, so a lifted row
    -- without them is one no route could have written.
    CONSTRAINT world_play_pauses_lifted_in_full CHECK (
        (lifted_at IS NULL) = (lifted_by IS NULL)
        AND (lifted_at IS NULL) = (lifted_by_name IS NULL)
        AND (lifted_at IS NULL) = (lift_grounds IS NULL)
    ),

    CONSTRAINT world_play_pauses_lift_grounds_are_not_blank CHECK (
        lift_grounds IS NULL OR length(btrim(lift_grounds)) > 0
    )
);

-- One active pause per world. Also the index the gate and the stream poll
-- read, which is why it is partial: a world paused and lifted a hundred times
-- still answers "is it paused now?" from at most one entry.
CREATE UNIQUE INDEX world_play_pauses_one_active
    ON world_play_pauses (world_id) WHERE lifted_at IS NULL;

-- What led to a request or a pause. Each trigger belongs to exactly one.
CREATE TABLE world_play_pause_triggers (
    id UUID PRIMARY KEY,

    request_id UUID REFERENCES world_play_pause_requests(id),
    pause_id UUID REFERENCES world_play_pauses(id),

    kind "PauseTriggerKind" NOT NULL,

    -- For a takedown: the `content_moderation_actions` row, and what it took
    -- down. No foreign key, because that table has none either.
    moderation_action_id UUID,
    entity_type TEXT,
    entity_id UUID,

    -- An operator's grounds, when a second operator pause lands on an active
    -- one (FR-036, FR-051).
    note TEXT,

    recorded_at TIMESTAMP NOT NULL DEFAULT now(),
    created_by UUID,

    CONSTRAINT world_play_pause_triggers_one_owner CHECK (
        num_nonnulls(request_id, pause_id) = 1
    ),

    CONSTRAINT world_play_pause_triggers_takedown_names_its_action CHECK (
        kind <> 'Takedown' OR moderation_action_id IS NOT NULL
    ),

    -- A retried takedown hook does not record the same action twice. NULLs
    -- are distinct, so operator triggers (no action) are not constrained.
    CONSTRAINT world_play_pause_triggers_once_per_request
        UNIQUE (moderation_action_id, request_id),
    CONSTRAINT world_play_pause_triggers_once_per_pause
        UNIQUE (moderation_action_id, pause_id)
);

-- Whether a world has been played in the last minute (research R3).
--
-- Durable because a takedown's decision must not depend on which process
-- filed it, and throttled so it is one write per world every 30 seconds rather
-- than one per client every five.
CREATE TABLE world_live_play (
    world_id UUID PRIMARY KEY REFERENCES worlds(id) ON DELETE CASCADE,
    last_beat_at TIMESTAMP NOT NULL
);

-- A pause may be lifted, once. Nothing else about it changes, and it is never
-- deleted.
--
-- The comparison is of the whole row less the columns a lift writes, so a
-- column added to this table later is protected without anyone remembering to
-- add it here.
CREATE FUNCTION world_play_pauses_only_lift() RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'a play pause is a record and is never deleted (spec 051 FR-053)';
    END IF;

    IF OLD.lifted_at IS NOT NULL THEN
        RAISE EXCEPTION 'a lifted play pause cannot change (spec 051): pause again instead';
    END IF;

    IF NEW.lifted_at IS NULL THEN
        RAISE EXCEPTION 'a play pause may only be lifted (spec 051)';
    END IF;

    IF (to_jsonb(NEW) - ARRAY['lifted_by', 'lifted_by_name', 'lifted_at', 'lift_grounds',
                              'updated_by', 'updated_at'])
       IS DISTINCT FROM
       (to_jsonb(OLD) - ARRAY['lifted_by', 'lifted_by_name', 'lifted_at', 'lift_grounds',
                              'updated_by', 'updated_at']) THEN
        RAISE EXCEPTION 'lifting a play pause may change nothing but the lift (spec 051)';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER world_play_pauses_only_lift_trigger
BEFORE UPDATE OR DELETE ON world_play_pauses
FOR EACH ROW EXECUTE PROCEDURE world_play_pauses_only_lift();

-- A decision is final (FR-035), and a request is never deleted — a declined
-- request is exactly the record an operator needs when the same world comes up
-- again.
CREATE FUNCTION world_play_pause_requests_decided_is_final() RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'a play pause request is a record and is never deleted (spec 051 FR-053)';
    END IF;

    IF OLD.state <> 'Pending' THEN
        RAISE EXCEPTION 'a decided play pause request cannot change (spec 051 FR-035): it was %',
            OLD.state;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER world_play_pause_requests_decided_is_final_trigger
BEFORE UPDATE OR DELETE ON world_play_pause_requests
FOR EACH ROW EXECUTE PROCEDURE world_play_pause_requests_decided_is_final();

-- A trigger records what happened. It is added, never edited, never removed.
CREATE FUNCTION world_play_pause_triggers_append_only() RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'what led to a play pause is a record and cannot be % (spec 051 FR-053)',
        CASE TG_OP WHEN 'DELETE' THEN 'deleted' ELSE 'changed' END;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER world_play_pause_triggers_append_only_trigger
BEFORE UPDATE OR DELETE ON world_play_pause_triggers
FOR EACH ROW EXECUTE PROCEDURE world_play_pause_triggers_append_only();
