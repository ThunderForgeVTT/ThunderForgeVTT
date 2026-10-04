-- Spec 039 T069, data-model.md § 5: the only stored piece of standing.
--
-- Strikes, whether an account may publish and whether it is disabled are
-- derived on every read. What cannot be derived is a *window*: when it opened,
-- when deletion falls due, and what was promised at the start of it. That is
-- this table, and nothing else is.

CREATE TABLE account_terminations (
    id UUID PRIMARY KEY,

    -- No foreign key to users: this row outlives the account it describes. A
    -- termination that ends in deletion is closed with reason 'deleted', and
    -- the row is the record that it happened and why.
    account_id UUID NOT NULL,

    opened_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Computed once, at open — the way `restoration_due_at` is computed once
    -- when a counter-notice is forwarded. Changing the window setting later
    -- does not move a window already running (FR-036).
    deletion_due_at TIMESTAMPTZ NOT NULL,

    strike_count_at_open INTEGER NOT NULL,

    -- A snapshot of MODERATION_TERMINATION_REQUIRES_HUMAN at open, so an
    -- operator flipping the setting does not change the terms somebody was
    -- told at the start of their window (FR-030, FR-036).
    requires_human BOOLEAN NOT NULL,

    -- False only for the instance's last administrator (FR-039): the row is
    -- written, a person must decide, and the account is NOT disabled — a
    -- product that locks itself out to enforce a rule has enforced nothing.
    disables_account BOOLEAN NOT NULL,

    appeal_state TEXT NOT NULL DEFAULT 'none'
        CHECK (appeal_state IN ('none', 'open', 'upheld', 'rejected')),
    -- The person's own words. One appeal per termination: a rejected appeal
    -- resumes the window from the date already passed, and re-filing would be
    -- a way to pause deletion forever.
    appeal_statement TEXT,
    appeal_filed_at TIMESTAMPTZ,
    appeal_resolved_at TIMESTAMPTZ,
    appeal_resolved_by UUID,
    appeal_note TEXT,

    closed_at TIMESTAMPTZ,
    -- 'counter_notice': the account filed one and, with that case no longer
    -- counting, fell below the threshold — the statutory route back, which a
    -- disabled account keeps (decided 2026-09-10).
    closed_reason TEXT
        CHECK (closed_reason IN (
            'appeal_upheld', 'strikes_aged_out', 'counter_notice', 'deleted', 'admin_reversed'
        )),

    -- A closed row says why; an open one says nothing.
    CHECK ((closed_at IS NULL) = (closed_reason IS NULL))
);

-- At most one open termination per account. Two windows for one account is
-- the bug that produces two deletion dates.
CREATE UNIQUE INDEX account_terminations_one_open
    ON account_terminations (account_id) WHERE closed_at IS NULL;

-- The two notices US7 adds beyond data-model.md's list: an account restored
-- without anybody asking (FR-035), and a player told where their character
-- went when the world it lived in was deleted with its creator's account.
ALTER TABLE account_notices DROP CONSTRAINT account_notices_kind_check;
ALTER TABLE account_notices ADD CONSTRAINT account_notices_kind_check CHECK (kind IN (
    'strike_recorded',
    'publishing_suspended',
    'account_disabled',
    'appeal_resolved',
    'account_restored',
    'actor_rescued',
    'share_taken_down',
    'adopted_copy_disabled',
    'adopted_copy_restored'
));
