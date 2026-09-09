-- Spec 041 FR-015 and FR-025: what happened to a second factor, and who did it.
--
-- Modelled on `instance_access_events` (spec 035), including its stated rule:
-- **the record describes the act, never the person.** No address, no user
-- agent, no email, no code and no fragment of one. A `recovery_code_used` row
-- says a code was used; it never says which.
--
-- `subject_user_id` and `actor_user_id` are separate columns, and that
-- separation is the whole point. FR-025 asks who did it *and* for whom, which
-- one column cannot answer. For a self-service enrolment they are equal; for an
-- operator reset they are not, and that is the row somebody will one day need.
--
-- `actor_user_id` is nullable and ON DELETE SET NULL: an operator who later
-- leaves the instance must not take the record of their reset with them. The
-- subject cascades, because an account that is gone has no security history to
-- show anyone.
CREATE TABLE two_factor_events (
    id UUID PRIMARY KEY,
    occurred_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    subject_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    CONSTRAINT two_factor_events_type_known
        CHECK (event_type IN (
            'enrolled',
            'removed',
            'recovery_code_used',
            'recovery_codes_issued',
            'reset_by_operator',
            'requirement_set',
            'requirement_cleared'
        ))
);

-- The two questions this table is asked: "what has happened to my account"
-- (the account holder, in their own security settings) and "what happened
-- lately" (an operator). One index per question.
CREATE INDEX two_factor_events_subject_idx
    ON two_factor_events(subject_user_id, occurred_at DESC);

CREATE INDEX two_factor_events_occurred_at_idx
    ON two_factor_events(occurred_at DESC);

COMMENT ON TABLE two_factor_events IS
  'Spec 041 FR-015/FR-025. Append-only. Records the act and never the person: no address, no user agent, no code. `subject_user_id` is whose factor changed; `actor_user_id` is who changed it, and they differ exactly when an operator acted.';
