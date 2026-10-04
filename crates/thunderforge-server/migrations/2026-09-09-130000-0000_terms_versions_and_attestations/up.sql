-- Spec 039 (T012), governed by ADR-076 (the sharing attestation record).
--
-- Two tables and one idea: **an agreement nobody recorded is not evidence.**
-- The terms are shown at the moment somebody publishes and the moment passes;
-- nothing today records who agreed, when, or to which words. If a copyright
-- notice arrives in eighteen months, "they clicked something" is not an answer.
--
-- Note what is deliberately absent, as the spec-026 collection migrations do:
-- there is no index and no query shape that lists attestations globally, by
-- version, or by world. `attestations_publishable_idx` answers "who agreed to
-- publish this named thing", which is the question a notice actually asks, and
-- `attestations_subject_idx` answers "what did I agree to". Nothing else.


-- The archive. Written at startup, before anything can name a version.
--
-- FR-016 — "an attestation always resolves to the words agreed to" — is a
-- property of two things together: this table is append-only, and
-- `legal::ensure_terms_versions_recorded` runs from main.rs before the server
-- serves. A version an attestation could name was archived before the server
-- that would accept the attestation was listening. There is no window.
--
-- NOTHING IS EVER UPDATED OR DELETED HERE. A row is a historical fact about
-- what a document said. An UPDATE would rewrite what people agreed to.
CREATE TABLE terms_versions (
    -- "<slug>@<first 16 hex of sha256(normalised body)>". Opaque: compared for
    -- equality and never parsed, ordered, or decomposed. See ADR-076 for why
    -- the identity is the hash of the words rather than a number somebody
    -- maintains — a maintained number is wrong the first time a typo is fixed
    -- without thinking about it, and wrong silently.
    version_id TEXT PRIMARY KEY,
    document_slug TEXT NOT NULL,
    -- The document as it was: comments stripped, trimmed. This is the copy an
    -- old attestation resolves through, which is why it is stored rather than
    -- recomputed from whatever the binary currently carries.
    body TEXT NOT NULL,
    first_seen_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- The only listing anybody needs: every version of one document, oldest first.
CREATE INDEX terms_versions_document_idx
    ON terms_versions(document_slug, first_seen_at);


-- One person, one moment, one version, one act.
--
-- Serves both the sharing attestation (US1-US4) and the operator
-- acknowledgement (US8). FR-043 says the operator record is made "on the same
-- terms as a sharing attestation"; one table is the only way that stays true as
-- either side changes.
CREATE TABLE attestations (
    id UUID PRIMARY KEY,

    -- 'share' | 'operator'. A CHECK rather than an enum type, matching
    -- `two_factor_events` and `content_moderation_actions`: a value added here
    -- without adding it to the constraint fails loudly at the INSERT.
    purpose TEXT NOT NULL,

    -- NO FOREIGN KEY TO users, and that is the decision rather than an
    -- omission.
    --
    -- This record has to outlive its subject. An account deletion must not be
    -- able to destroy the evidence for a claim already filed against it, and a
    -- foreign key would force the choice between failing the deletion and
    -- cascading the record away. Both are wrong. The precedent is
    -- `content_moderation_actions`, which points at content without
    -- constraining it, for the same reason.
    --
    -- What deletion does instead: `subject_username` becomes NULL and nothing
    -- else changes (FR-010, FR-037). What remains is "somebody, identified only
    -- by an id no longer joinable to a person, agreed to this exact text at
    -- this exact time in order to publish this exact thing" — which is what a
    -- notice needs, and names nobody.
    subject_user_id UUID NOT NULL,
    subject_username TEXT,

    -- The ONE foreign key in this feature, and it has to be one: an attestation
    -- naming words that are gone is precisely the failure the archive exists to
    -- prevent. Because the archive is append-only, this constraint can never
    -- block a write.
    terms_version_id TEXT NOT NULL REFERENCES terms_versions(version_id),

    -- 'collection' | 'actor' | 'item' | 'ability'; NULL when purpose='operator'.
    -- Polymorphic, so no foreign key is possible here either — the same shape
    -- `world_collection_members.member_id` already uses.
    publishable_kind TEXT,
    publishable_id UUID,

    -- The share row minted in the same transaction. A plain uuid with no
    -- foreign key **on purpose**: revoking or deleting the share leaves this
    -- record standing (FR-007). It is a pointer, not a dependency.
    share_id UUID,

    world_id UUID,

    attested_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT attestations_purpose_check
        CHECK (purpose IN ('share', 'operator')),

    CONSTRAINT attestations_publishable_kind_check
        CHECK (
            publishable_kind IS NULL
            OR publishable_kind IN ('collection', 'actor', 'item', 'ability')
        ),

    -- A share attestation is about something; an operator acknowledgement is
    -- about the instance. Stating it here rather than trusting every writer
    -- means a row that describes neither cannot exist.
    CONSTRAINT attestations_shape_check
        CHECK (
            (purpose = 'share'
                AND publishable_kind IS NOT NULL
                AND publishable_id IS NOT NULL)
            OR
            (purpose = 'operator'
                AND publishable_kind IS NULL
                AND publishable_id IS NULL
                AND world_id IS NULL
                AND share_id IS NULL)
        )
);

-- The lookup a person handling a notice makes: who agreed to publish this named
-- thing, newest first (FR-009, SC-003).
CREATE INDEX attestations_publishable_idx
    ON attestations(publishable_kind, publishable_id, attested_at DESC);

-- A person's own history, newest first. Available even to a disabled account —
-- somebody shut out should still be able to see what they agreed to.
CREATE INDEX attestations_subject_idx
    ON attestations(subject_user_id, attested_at DESC);

-- "Who agreed to this version" — asked when a version turns out to have said
-- something it should not have.
CREATE INDEX attestations_terms_version_idx
    ON attestations(terms_version_id);

-- At most one operator acknowledgement in force per instance. There is one
-- instance per database, so this is a partial unique index over a constant.
CREATE UNIQUE INDEX attestations_one_operator_idx
    ON attestations((TRUE)) WHERE purpose = 'operator';
