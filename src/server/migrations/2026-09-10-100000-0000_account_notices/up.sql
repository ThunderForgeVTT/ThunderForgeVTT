-- Spec 039 T017, data-model.md § 6: what a person has been told about their
-- own account.
--
-- There is no notifications table, no mailer and no queue anywhere else in
-- this codebase; spec 040 owns delivery. This is the durable half of "the
-- person is told" (FR-028): a row exists because the telling happened, and
-- when mail arrives it delivers these rows. The table is the queue.

CREATE TABLE account_notices (
    id UUID PRIMARY KEY,

    -- No foreign key to users, for the reason `attestations` gives: nothing in
    -- this feature points at a user with a constraint that would force a choice
    -- between failing a deletion and cascading a record away. Account deletion
    -- removes these rows explicitly, in its own transaction (FR-037) — a
    -- notice is addressed to a person, and says nothing once they are gone.
    account_id UUID NOT NULL,

    -- A CHECK rather than an enum type, matching `attestations` and
    -- `content_moderation_actions`: a kind added in code and not here fails
    -- loudly at the INSERT.
    kind TEXT NOT NULL CHECK (kind IN (
        'strike_recorded',
        'publishing_suspended',
        'account_disabled',
        'appeal_resolved',
        'share_taken_down',
        'adopted_copy_disabled',
        'adopted_copy_restored'
    )),

    -- What it was about: case, entity, world.
    subject_ref JSONB,

    -- The values the rendered text needs. The TEXT IS NOT STORED: `kind` plus
    -- `payload` is rendered by the client, so a wording fix does not require
    -- rewriting history. The opposite choice from `attestations`, deliberately
    -- — an attestation's value is that the exact words are fixed; a notice's
    -- value is that it was produced.
    payload JSONB,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- For a badge, not for cleanup. Rows are never deleted on read.
    read_at TIMESTAMPTZ
);

-- The only read: one account's notices, newest first.
CREATE INDEX account_notices_account_idx ON account_notices (account_id, created_at DESC);
