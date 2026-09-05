-- Spec 034, User Story 3: changes observed in a repository that a world does
-- not have, waiting for a person to decide about them.
--
-- This table is deliberately absent from the first delivery's migration
-- (2026-09-04-120000-0000_lore_repository_sync). Stories 1 and 2 have no
-- writer for it, and a table nothing writes is a table that lies about what
-- the system does.
--
-- # What this table is for, in one sentence
--
-- It is the gap between "the repository says something different" and "the
-- world changed", and it exists so that the gap is a ROW A HUMAN DECIDES ON
-- rather than an implicit merge. Everything else here follows from that.
--
-- Up to this migration, nothing in `lore_sync` could damage a world by
-- construction: no path in it wrote to any lore table. This story removes that
-- guarantee, so each rule that replaces it is written into the schema where a
-- future code path cannot forget it, rather than into a function that a future
-- code path can decline to call.
CREATE TABLE lore_pending_incoming_changes (
    id UUID PRIMARY KEY,

    -- The connection, not the world. Deleting a connection (FR-005) must leave
    -- the world's lore entirely intact, and the correct thing to discard along
    -- with the connection is precisely the set of undecided proposals that came
    -- from the repository it named. CASCADE says so.
    connection_id UUID NOT NULL REFERENCES lore_repository_connections(id) ON DELETE CASCADE,

    -- Which entry this concerns, or NULL for "this proposes a new one".
    --
    -- NULL is FR-027 in the schema: a file carrying no durable identifier we
    -- recognise is a PROPOSED NEW ENTRY, never matched to an existing entry by
    -- path or title. There is no column here that could hold a guess, because a
    -- guess is what FR-027 forbids and a nullable "probably this entry" column
    -- is how that rule would eventually be broken.
    lore_entry_id UUID REFERENCES world_lore_entries(id) ON DELETE CASCADE,

    kind TEXT NOT NULL,
    CONSTRAINT lore_pending_incoming_kind_known CHECK (
        kind IN ('update', 'new_entry', 'deletion')
    ),

    -- Where the file was seen. A LABEL for a human reviewing the change, in the
    -- same sense as `lore_exported_entries.current_path` — never a key, never
    -- consulted to decide which entry a file is about (FR-027).
    repository_path TEXT NOT NULL,

    -- The title the file's header or filename suggests, for a proposed new
    -- entry. Present so a reviewer sees a name rather than a path; it names
    -- nothing that already exists.
    proposed_title TEXT,

    -- The incoming markdown, in AUTHORED form — repository link syntax already
    -- turned back into the app's own. Stored rather than re-read from a clone
    -- because the decision may be taken days after the pass that detected it,
    -- by which time the clone is gone and the branch has moved on. A proposal
    -- must show the reviewer the text they are agreeing to, not the text the
    -- repository happens to hold when they click.
    incoming_body TEXT,

    -- The revision the exported file was built from: the common ancestor of
    -- the two texts. Kept so that "changed on both sides" is a claim about
    -- recorded history rather than about a timestamp comparison.
    base_revision_id UUID REFERENCES world_lore_revisions(id) ON DELETE SET NULL,

    -- The entry's current revision in the app at the moment of detection. With
    -- `base_revision_id` this is the whole of FR-024's evidence: the two
    -- versions to present.
    app_revision_id UUID REFERENCES world_lore_revisions(id) ON DELETE SET NULL,

    -- FR-024. A stored answer, not a derived one, because it is an observation
    -- about a moment: whether the entry had also moved on in the app when this
    -- change was seen. Recomputing it later from current state would silently
    -- change what the reviewer was told.
    --
    -- There is no `merged_body` column and there must never be one. FR-024
    -- forbids merging prose automatically at any priority, and the absence of
    -- anywhere to put a merge is the strongest form that rule can take.
    also_changed_in_app BOOLEAN NOT NULL DEFAULT FALSE,

    -- FR-023 and FR-026 in one column. A row is inert until somebody moves it
    -- off 'pending'; nothing else in the system reads an undecided row as an
    -- instruction.
    status TEXT NOT NULL DEFAULT 'pending',
    CONSTRAINT lore_pending_incoming_status_known CHECK (
        status IN ('pending', 'accepted', 'declined')
    ),

    detected_at TIMESTAMP NOT NULL DEFAULT NOW(),
    decided_at TIMESTAMP,
    -- FR-023: an acceptance is by a named user holding authority, and the
    -- record of who is not optional once a decision exists.
    decided_by UUID REFERENCES users(id),

    -- FR-025's mark of origin.
    --
    -- An accepted change becomes an ORDINARY revision in `world_lore_revisions`
    -- — same table, same history, same restore behaviour — and this column is
    -- what makes it identifiable as having come from the repository. The mark
    -- lives here rather than as a column on `world_lore_revisions` on purpose:
    -- adding a nullable `origin` to the revisions table would put a
    -- synchronisation concept inside the lore model that every unrelated
    -- reader and writer of revisions would then have to understand. A revision
    -- is repository-originated exactly when a row here points at it, which is
    -- queryable, joins in one hop, and leaves the lore schema as it was.
    applied_revision_id UUID REFERENCES world_lore_revisions(id) ON DELETE SET NULL,

    -- Which entry an accepted *proposal for a new entry* turned into.
    --
    -- A separate column rather than filling in `lore_entry_id`, because
    -- `lore_entry_id` carries FR-027's meaning — "this proposal concerns an
    -- entry that already existed" — and writing the newly created entry into it
    -- would make an accepted proposal indistinguishable, a month later, from a
    -- file that had been matched to an existing entry. The rule FR-027 states
    -- is worth being able to audit after the fact, not only at the moment it is
    -- applied.
    created_entry_id UUID REFERENCES world_lore_entries(id) ON DELETE SET NULL,

    -- FR-027 again, this time as an invariant a bad INSERT cannot slip past:
    -- a proposal for a new entry names no entry, and a change to an existing
    -- entry names one.
    CONSTRAINT lore_pending_incoming_new_entry_names_no_entry CHECK (
        (kind = 'new_entry') = (lore_entry_id IS NULL)
    ),

    -- A deletion proposes no text, and everything else does. A deletion row
    -- carrying a body would be a row that could be accepted as an edit.
    CONSTRAINT lore_pending_incoming_body_matches_kind CHECK (
        (kind = 'deletion') = (incoming_body IS NULL)
    ),

    -- A decision has a decider and a time, and an undecided row has neither.
    -- Without this, "accepted by nobody at no time" is representable, and the
    -- audit trail FR-023 depends on has a hole in it.
    CONSTRAINT lore_pending_incoming_decision_is_complete CHECK (
        (status = 'pending' AND decided_at IS NULL AND decided_by IS NULL)
        OR (status <> 'pending' AND decided_at IS NOT NULL AND decided_by IS NOT NULL)
    ),

    -- Nothing was applied unless it was accepted (FR-023). A declined or
    -- pending row pointing at a revision would mean lore had been written
    -- without an acceptance, which is the single thing this whole story must
    -- not allow.
    CONSTRAINT lore_pending_incoming_applied_only_when_accepted CHECK (
        applied_revision_id IS NULL OR status = 'accepted'
    ),

    -- An entry can only have been created by an accepted proposal for a new
    -- entry. Anything else here would mean an entry appeared in a world
    -- because of a file, without anyone accepting it.
    CONSTRAINT lore_pending_incoming_created_only_by_accepted_new_entry CHECK (
        created_entry_id IS NULL OR (kind = 'new_entry' AND status = 'accepted')
    )
);

-- One undecided proposal per entry. A second detection pass observing the same
-- divergence must update the existing row rather than stack a second one: two
-- pending rows for one entry means two accept buttons for one entry, and
-- pressing both writes the older text last.
--
-- Partial, on `status = 'pending'`, so that a long history of decided rows for
-- one entry stays legal — that history is the record of what the repository
-- has proposed over time.
CREATE UNIQUE INDEX lore_pending_incoming_one_per_entry
    ON lore_pending_incoming_changes (connection_id, lore_entry_id)
    WHERE status = 'pending' AND lore_entry_id IS NOT NULL;

-- The same for a proposed new entry, which has no entry to key on. The path is
-- used here as an identifier of the FILE, which is all a not-yet-existing entry
-- has — this is not FR-027 matching, because there is nothing to match it to.
CREATE UNIQUE INDEX lore_pending_incoming_one_per_new_file
    ON lore_pending_incoming_changes (connection_id, repository_path)
    WHERE status = 'pending' AND lore_entry_id IS NULL;

-- The review surface's query: everything undecided for one connection, newest
-- first.
CREATE INDEX lore_pending_incoming_connection_status
    ON lore_pending_incoming_changes (connection_id, status, detected_at DESC);
