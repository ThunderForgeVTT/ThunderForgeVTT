-- Spec 036 (T005), governed by ADR-073 (concurrent sessions and the single
-- play-field claim).
--
-- `user_sessions` has always been able to hold several live rows for one
-- account — it is keyed by `id` with a plain `user_id` and no unique
-- constraint. Only `create_session` prevented it, by revoking every
-- un-revoked row on each login. ADR-073 removes that, and these columns are
-- what replaces the protection it was providing: a person can see their own
-- sessions, recognise them, and end the one they do not know.
--
-- Nothing here lengthens a session or weakens an existing check. `revoked_at`
-- and `expires_at` keep exactly the meanings they have.

-- When this session was last used. Needed for two things a login-time revoke
-- made unnecessary: telling one session from another in a list, and choosing
-- which to evict when an account reaches the concurrent bound.
--
-- Backfilled from `created_at` rather than `now()`: a session created a week
-- ago and unused since should not read as active today.
ALTER TABLE user_sessions
    ADD COLUMN last_seen_at TIMESTAMP;
UPDATE user_sessions SET last_seen_at = created_at WHERE last_seen_at IS NULL;
ALTER TABLE user_sessions
    ALTER COLUMN last_seen_at SET NOT NULL,
    ALTER COLUMN last_seen_at SET DEFAULT CURRENT_TIMESTAMP;

-- A coarse, human-recognisable origin: browser family and platform as the
-- client reported them, never an address. Spec 035 set the precedent that an
-- access record describes the act and not the person, and a session list is
-- read by its owner for exactly one purpose — "is that one me?" — which a
-- browser and a platform answer and an IP address does not.
ALTER TABLE user_sessions
    ADD COLUMN client_description TEXT;

-- Why a session ended. `revoked_at` records that it did; this records which
-- of the several reasons applied, which matters once ending a session is
-- something a person does deliberately rather than a side effect of logging
-- in somewhere else.
ALTER TABLE user_sessions
    ADD COLUMN ended_reason TEXT;

ALTER TABLE user_sessions
    ADD CONSTRAINT user_sessions_ended_reason_known
    CHECK (ended_reason IS NULL OR ended_reason IN (
        'signed_out',
        'ended_by_user',
        'password_changed',
        'bound_exceeded',
        'expired'
    ));

-- The list a person reads is "my live sessions, most recently used first",
-- and the eviction choice is "my least recently used". One index serves both.
CREATE INDEX user_sessions_user_last_seen_idx
    ON user_sessions(user_id, last_seen_at DESC)
    WHERE revoked_at IS NULL;
