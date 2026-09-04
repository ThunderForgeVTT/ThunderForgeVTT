-- Spec 034: optional lore synchronisation to an external repository.
--
-- Four tables plus one for disassociation notices. The first delivery is
-- export only (Stories 1 and 2), so nothing here is written by a path that
-- also writes a world's lore — that is what makes "a first delivery cannot
-- damage a world" true by construction rather than by care.

-- One connection per world (FR-001). The world's link to one repository.
CREATE TABLE lore_repository_connections (
    id UUID PRIMARY KEY,
    -- The uniqueness constraint IS FR-001. Enforcing "at most one connection
    -- per world" in application code would be enforcing it nowhere.
    world_id UUID NOT NULL UNIQUE REFERENCES worlds(id) ON DELETE CASCADE,

    -- Which host adapter arranged the grant, and its opaque handle. Read at
    -- the grant boundary and NOWHERE else (FR-004c) — no component past the
    -- grant may branch on either. Neither is a credential; the token derived
    -- from the installation is short-lived and never persisted (FR-036d),
    -- which is why this table has no credential column and is therefore safe
    -- to read in full when diagnosing a connection.
    host_kind TEXT NOT NULL,
    installation_ref TEXT NOT NULL,

    repository_ref TEXT NOT NULL,
    branch TEXT NOT NULL,
    directory TEXT NOT NULL,

    -- Story 3's gate. FR-006 and FR-022 both depend on the default being off:
    -- a world must be able to synchronise outward with acceptance of incoming
    -- changes never enabled.
    incoming_enabled BOOLEAN NOT NULL DEFAULT FALSE,

    -- FR-038: synchronisation MUST NOT begin until the Game Master has
    -- acknowledged FR-037's notice. NULL means never started, and the
    -- background task skips such a row entirely.
    notice_acknowledged_at TIMESTAMP,

    -- FR-029's three words exactly, plus one. `deactivated` is an enforcement
    -- action (FR-041a) and is the only state a Game Master cannot leave by
    -- fixing something — a deactivation the owner could undo is not one.
    state TEXT NOT NULL DEFAULT 'never_configured',
    CONSTRAINT lore_repository_connections_state_known CHECK (
        state IN ('working', 'needs_attention', 'never_configured', 'deactivated')
    ),
    -- Always present on needs_attention. A state that says something is wrong
    -- without saying what sends a Game Master to a support channel, which
    -- FR-029 exists to prevent. Plain language, never a raw host error.
    state_reason TEXT,

    -- FR-040a. Observed at the last run, not guaranteed: visibility changes at
    -- the host without telling us, so anywhere this is shown must say when it
    -- was last seen. NULL before the first run.
    repository_is_public BOOLEAN,
    visibility_checked_at TIMESTAMP,

    deactivated_at TIMESTAMP,
    deactivated_reason TEXT,

    last_synced_at TIMESTAMP,
    -- What we believe the remote head to be. FR-031 compares against this to
    -- detect that the history no longer contains what we wrote.
    last_written_commit TEXT,

    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),

    -- FR-033: two worlds MUST NOT synchronise into the same directory of the
    -- same repository. Also a database constraint for the same reason as
    -- FR-001 above.
    CONSTRAINT lore_repository_connections_one_world_per_directory
        UNIQUE (repository_ref, directory)
);

-- One attempt to bring a repository into agreement with a world.
--
-- Retained rather than overwritten, because FR-030's backoff and FR-029's
-- "notify once rather than repeatedly" are both statements about a HISTORY of
-- attempts. A single mutable status column cannot express either.
CREATE TABLE lore_sync_runs (
    id UUID PRIMARY KEY,
    connection_id UUID NOT NULL REFERENCES lore_repository_connections(id) ON DELETE CASCADE,
    started_at TIMESTAMP NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMP,
    outcome TEXT,
    CONSTRAINT lore_sync_runs_outcome_known CHECK (
        outcome IS NULL OR outcome IN (
            'succeeded', 'failed', 'stopped_for_divergence', 'stopped_for_collision'
        )
    ),
    from_commit TEXT,
    to_commit TEXT,
    entries_written INTEGER NOT NULL DEFAULT 0,
    -- In terms a Game Master can act on, not a stack trace.
    failure_reason TEXT,
    -- Drives FR-030's progressively longer intervals.
    attempt INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX lore_sync_runs_connection_started
    ON lore_sync_runs (connection_id, started_at DESC);

-- The durable association between an entry and the file representing it.
--
-- Without this a rename is indistinguishable from a delete plus an unrelated
-- create, and FR-010's history preservation is impossible. The id in the file
-- header lets an INCOMING file be matched (FR-009, FR-027); this row is what
-- lets the OUTGOING side know which file to move. Two directions of the same
-- identity, both needed.
CREATE TABLE lore_exported_entries (
    id UUID PRIMARY KEY,
    connection_id UUID NOT NULL REFERENCES lore_repository_connections(id) ON DELETE CASCADE,
    lore_entry_id UUID NOT NULL REFERENCES world_lore_entries(id) ON DELETE CASCADE,
    -- Relative to the connection's directory. A LABEL, never a key — the
    -- entry id in the file header is the key (FR-009).
    current_path TEXT NOT NULL,
    -- Which revision the file currently carries. How "changed on both sides"
    -- (FR-024) is answered.
    exported_revision_id UUID REFERENCES world_lore_revisions(id) ON DELETE SET NULL,
    last_exported_at TIMESTAMP,

    -- FR-007: exactly one file per entry.
    CONSTRAINT lore_exported_entries_one_file_per_entry
        UNIQUE (connection_id, lore_entry_id),
    -- Two entries may not claim one path.
    CONSTRAINT lore_exported_entries_one_entry_per_path
        UNIQUE (connection_id, current_path)
);

-- Something that could not be represented (FR-013, FR-037).
--
-- Rows, not log lines: SC-008 requires every fidelity loss to be ENUMERATED
-- rather than discovered by the user, and something enumerable must be
-- queryable.
CREATE TABLE lore_fidelity_notes (
    id UUID PRIMARY KEY,
    connection_id UUID NOT NULL REFERENCES lore_repository_connections(id) ON DELETE CASCADE,
    -- NULL for a note about the whole connection, such as permission
    -- flattening or the fact that the mirror is public.
    lore_entry_id UUID REFERENCES world_lore_entries(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    CONSTRAINT lore_fidelity_notes_kind_known CHECK (
        kind IN (
            'unresolvable_cross_link',
            'permission_not_carried',
            'path_disambiguated',
            'mirrored_publicly'
        )
    ),
    detail TEXT NOT NULL,
    first_seen_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX lore_fidelity_notes_connection ON lore_fidelity_notes (connection_id);

-- One attempt to lodge a public withdrawal after a takedown (FR-040b).
--
-- A table rather than a log line because FR-040d says a failure must not block
-- or reverse the takedown and must reach an administrator, and "did we, for
-- this takedown, and if not why" needs an answer a year later.
--
-- `skipped_private` is recorded rather than omitted, so that "we deliberately
-- did not do this, and here is why" and "we forgot" never look the same.
CREATE TABLE lore_disassociation_notices (
    id UUID PRIMARY KEY,
    connection_id UUID NOT NULL REFERENCES lore_repository_connections(id) ON DELETE CASCADE,
    moderation_action_id UUID NOT NULL,
    attempted_at TIMESTAMP NOT NULL DEFAULT NOW(),
    outcome TEXT NOT NULL,
    CONSTRAINT lore_disassociation_notices_outcome_known CHECK (
        outcome IN ('lodged', 'failed', 'skipped_private')
    ),
    -- Where it landed, so it can be pointed at.
    issue_ref TEXT,
    failure_reason TEXT
);

CREATE INDEX lore_disassociation_notices_connection
    ON lore_disassociation_notices (connection_id, attempted_at DESC);
