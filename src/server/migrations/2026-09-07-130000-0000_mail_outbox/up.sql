-- Spec 040 US4 (FR-013 – FR-017): the outbox.
--
-- One row per message this instance means to send, written BEFORE anything
-- tries to send it. That ordering is the whole design: FR-015 says an instance
-- with no mail configured must not silently discard a message, and the only
-- way to keep that promise by construction rather than by discipline is for
-- the durable record to exist before the transport is consulted at all.
-- Nothing in the server calls a transport directly; a feature that wants to
-- tell somebody something inserts a row here and the background sender does
-- the rest.
--
-- `state` carries `blocked` as well as `failed`, and the difference between
-- them is the point (data-model.md § 3): `blocked` means "this instance cannot
-- send yet, and configuring mail releases it"; `failed` means "it was tried
-- and the retry curve is exhausted". A single `failed` would make "we never
-- had a mail server" indistinguishable from "the mail server refused us",
-- which are different sentences to an operator and different fixes.
--
-- Both `subject` and `body` are stored as ciphertext (`v1.<nonce>.<ct>`,
-- crypto.rs) because a retry must send the message that failed rather than a
-- re-rendered approximation of it, and because FR-016 forbids the contents of
-- somebody's message reaching anyone it was not for. Honest boundary, recorded
-- in research.md § D5: this defends the admin surface, the logs and a leaked
-- backup — not the operator, who holds THUNDERFORGE_SECRET and the database.
--
-- There is deliberately no column for a rendered HTML body, no template id and
-- no per-feature payload. This feature owns whether a message can be sent, not
-- what any message says; specs 035, 037 and 039 own theirs.
CREATE TABLE mail_outbox (
    id UUID PRIMARY KEY,
    -- 'test' today, and whatever 035/037/039 name later. Text rather than an
    -- enum so a new caller does not need a migration to say why it wrote.
    purpose TEXT NOT NULL,
    to_address TEXT NOT NULL,
    subject_encrypted TEXT NOT NULL,
    body_encrypted TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'queued',
    attempts INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TIMESTAMP,
    -- When the backoff curve next allows an attempt. NULL means "now".
    next_attempt_at TIMESTAMP,
    -- Operator-facing prose naming what to fix. Never a password, never a
    -- fragment of one, never its length (FR-014).
    last_failure_reason TEXT,
    sent_at TIMESTAMP,
    -- Nullable (Principle III): the instance itself enqueues messages with no
    -- actor behind them. A null renders as "the instance", never as a blank.
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT mail_outbox_state_known
        CHECK (state IN ('queued', 'blocked', 'sending', 'sent', 'failed')),
    -- `sent_at` is the idempotence guard a retry checks (contracts/mail.md
    -- rule 6), so the database refuses the two ways it could lie: a sent row
    -- with no timestamp, and a timestamp on a row claiming not to have sent.
    CONSTRAINT mail_outbox_sent_has_a_time
        CHECK ((state = 'sent') = (sent_at IS NOT NULL))
);

-- The sender's question every tick is "what is due now", which reads by state
-- and then by time; the operator's question is "what happened lately", which
-- reads newest-first. Two indexes because they are two different questions.
CREATE INDEX mail_outbox_state_next_attempt_idx
    ON mail_outbox(state, next_attempt_at NULLS FIRST);
CREATE INDEX mail_outbox_created_at_idx
    ON mail_outbox(created_at DESC);
