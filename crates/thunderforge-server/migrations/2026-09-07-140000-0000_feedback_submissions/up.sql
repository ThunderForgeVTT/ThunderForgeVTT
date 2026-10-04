-- Spec 037 (FR-017 – FR-022): the submission is the record; the tracker is a
-- destination.
--
-- FR-018 decides the shape of all four tables. It says the instance records a
-- submission BEFORE any delivery is attempted, which makes the durable object
-- the submission and GitHub somewhere a copy of it is later sent. The
-- alternative — call the host in the resolver and keep the issue URL — makes
-- the host the store, and then FR-030 ("with no destination configured at all,
-- submissions MUST still be kept") has nowhere to keep anything.
--
-- Modelled on `lore_repository_connections` / `lore_sync_runs`, deliberately:
-- one row per destination, one retained row per attempt, and a visibility
-- observation that is always shown with the time it was made.

CREATE TABLE feedback_submissions (
    id UUID PRIMARY KEY,
    -- FR-013's "traceable *inside* the instance". Never sent anywhere: the
    -- issue body carries the submission id and nothing about the person.
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    -- What the person typed, verbatim. Never redacted — spec.md's Edge Cases
    -- settle it: what somebody deliberately types is theirs.
    message TEXT NOT NULL,
    summary TEXT,
    screen_path TEXT,
    -- SET NULL rather than CASCADE: a deleted world must not delete a report
    -- about it.
    world_id UUID REFERENCES worlds(id) ON DELETE SET NULL,
    -- Resolved server-side from world_id, never accepted from the client
    -- (research.md § R13). A client that could name the system could name a
    -- different world's.
    game_system_id TEXT,
    client_version TEXT NOT NULL,
    -- Recorded alongside the client's, because a stale cached bundle against a
    -- new server is a real and diagnosable situation that is invisible unless
    -- both are captured.
    server_version TEXT NOT NULL,
    -- Coarse family and platform. Never the full User-Agent, never an address.
    browser TEXT NOT NULL,
    delivery_state TEXT NOT NULL DEFAULT 'pending',
    -- research.md § R4. Written into the issue body; the handle by which a
    -- duplicate — if the one unavoidable case ever produces one — is found.
    delivery_key UUID NOT NULL UNIQUE,
    issue_url TEXT,
    issue_number INTEGER,
    issue_state TEXT,
    -- Shown WITH the state, for the same reason visibility_checked_at is: a
    -- maintainer closes an issue without telling us.
    issue_state_checked_at TIMESTAMP,
    -- A column, not a computation, so the retention a person was promised is
    -- the retention that was in force when they submitted; changing the
    -- constant later cannot retroactively shorten it (FR-016).
    attachments_expire_at TIMESTAMP NOT NULL,
    -- Set by the sweep. Distinguishes "expired and removed" from "expired and
    -- the sweep has not run", which a person asking where their screenshot
    -- went deserves.
    attachments_purged_at TIMESTAMP,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    updated_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- FR-002's exactly three, enforced by the database rather than by a
    -- resolver: a fourth kind cannot be introduced by a typo.
    CONSTRAINT feedback_submissions_kind_known
        CHECK (kind IN ('feature_request', 'issue', 'general')),
    CONSTRAINT feedback_submissions_delivery_state_known
        CHECK (delivery_state IN ('pending', 'delivered', 'abandoned')),
    -- The only two states the tracker has. Inventing a third would be this
    -- product guessing at a maintainer's meaning (research.md § R14).
    CONSTRAINT feedback_submissions_issue_state_known
        CHECK (issue_state IS NULL OR issue_state IN ('open', 'closed'))
);

-- Three questions, three indexes. `mine` is US5's list, `pending` is the
-- delivery pass, `expiring` is the retention sweep — and the last two are
-- partial so neither scans delivered history every thirty seconds.
CREATE INDEX feedback_submissions_mine
    ON feedback_submissions (user_id, created_at DESC);
CREATE INDEX feedback_submissions_pending
    ON feedback_submissions (delivery_state, created_at)
    WHERE delivery_state = 'pending';
CREATE INDEX feedback_submissions_expiring
    ON feedback_submissions (attachments_expire_at)
    WHERE attachments_purged_at IS NULL;

-- One row per approved attachment.
--
-- NO content_hash and no dedupe lookup, and that omission is load-bearing.
-- `storage/dedupe.rs` states that adding object deletion means adding
-- reference counting first; the reason feedback may delete without it is that
-- no second row can ever name a feedback object. Leaving the column out is
-- what makes that checkable rather than promised.
--
-- No created_by/updated_by: an attachment is owned entirely by its submission,
-- which carries both, and ON DELETE CASCADE means it has no independent life.
CREATE TABLE feedback_attachments (
    id UUID PRIMARY KEY,
    submission_id UUID NOT NULL REFERENCES feedback_submissions(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    -- feedback/{submission_id}/{id}.{webp|log} — computed server-side, never
    -- client-supplied, on the rule `rustfs::object_key` already states.
    storage_path TEXT NOT NULL,
    -- 'image/webp' or 'text/plain'. The latter is this storage layer's first
    -- non-image object, which is worth saying out loud.
    content_type TEXT NOT NULL,
    byte_size BIGINT NOT NULL,
    entries_kept INTEGER,
    entries_dropped INTEGER,
    redaction_count INTEGER NOT NULL DEFAULT 0,
    -- Set when the object is deleted. The row survives its bytes, so a
    -- submission can still say what was attached.
    purged_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT feedback_attachments_kind_known
        CHECK (kind IN ('logs', 'screenshot'))
);

CREATE INDEX feedback_attachments_submission
    ON feedback_attachments (submission_id);

-- One try, retained rather than overwritten — for the reason `lore_sync_runs`
-- gives about itself: the backoff and the operator's view are both statements
-- about a HISTORY of attempts, and a single mutable status column cannot
-- express either.
CREATE TABLE feedback_delivery_attempts (
    id UUID PRIMARY KEY,
    submission_id UUID NOT NULL REFERENCES feedback_submissions(id) ON DELETE CASCADE,
    attempt INTEGER NOT NULL DEFAULT 1,
    started_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- NULL means in flight. This is the single-flight mechanism (research.md
    -- § R4): `due_now` refuses a submission whose latest attempt has not
    -- finished, exactly as lore sync's selection refuses a connection whose
    -- latest run has a null outcome.
    finished_at TIMESTAMP,
    outcome TEXT,
    -- In terms an operator can act on, never a raw host body. FR-021 forbids
    -- any credential or fragment of one here, so the delivery code maps host
    -- errors to a fixed vocabulary rather than passing text through.
    failure_reason TEXT,
    -- Nullable, and its absence is exactly the ambiguous case: no status means
    -- the host may have acted, which is what makes the next attempt search
    -- before it creates.
    http_status INTEGER,
    -- 'adopted' is worth its own value rather than folding into 'created': it
    -- records that a retry found an issue an earlier ambiguous attempt had
    -- already made, which is the evidence FR-019 held under the one condition
    -- that could break it.
    CONSTRAINT feedback_delivery_attempts_outcome_known
        CHECK (outcome IS NULL OR outcome IN ('created', 'adopted', 'failed', 'not_configured'))
);

CREATE INDEX feedback_delivery_attempts_submission
    ON feedback_delivery_attempts (submission_id, started_at DESC);

-- At most one row: the instance's destination, not a world's.
--
-- No credential column, deliberately, matching `lore_repository_connections`.
-- The installation token is short-lived and derived per call, so this table is
-- safe to read in full when diagnosing a destination — which is the property
-- FR-021 depends on: an operator view that cannot leak a secret because there
-- is no secret in the rows it reads.
CREATE TABLE feedback_destination (
    id UUID PRIMARY KEY,
    installation_ref TEXT NOT NULL,
    repository_ref TEXT NOT NULL,
    attachment_branch TEXT NOT NULL DEFAULT 'feedback-attachments',
    -- Observed, never assumed (FR-014). NULL means never checked, and the
    -- notice says so rather than reassuring.
    is_public BOOLEAN,
    visibility_checked_at TIMESTAMP,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- "At most one row" as a constraint rather than as a convention. A second
-- destination would make "where does feedback go" a question with two answers
-- and no rule for choosing.
CREATE UNIQUE INDEX feedback_destination_singleton
    ON feedback_destination ((TRUE));
