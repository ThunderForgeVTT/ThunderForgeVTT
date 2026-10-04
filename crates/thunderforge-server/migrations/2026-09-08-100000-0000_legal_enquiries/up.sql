-- Terms-of-service disputes and privacy requests, as an intake queue.
--
-- # Why this is not `content_moderation_actions`
--
-- That table is the DMCA case file, and its columns are statutory: the
-- copyrighted work, the good-faith statement, the accuracy statement, the
-- signature. Its side effect is disabling the targeted content. A person
-- disputing the terms of service is not naming a work, is not swearing to
-- anything under penalty of perjury, and — critically — must not cause
-- anything to be taken down. Putting them in the same table would mean either
-- eight columns that are always null for two of the three kinds, or a takedown
-- path that can be entered without the elements that make a takedown lawful.
--
-- So: a sibling. The admin surface shows all three together, which is the part
-- an operator actually wanted unified.
--
-- # Why the submitter is not required to have an account
--
-- The same reason spec 039 FR-056 gives for the notice contact: somebody who
-- objects to the terms of a service may well be objecting to the terms they
-- were asked to accept, and a complaints channel that requires accepting them
-- first is not a complaints channel. `submitted_by` records an account when
-- there is one and is null otherwise.
--
-- # What is deliberately absent
--
-- Any column recording the submitter's address or their IP. The rate limiter
-- sees the address and does not store it — spec 035's "record the act, never
-- the person". What is stored is what the person chose to type.
CREATE TABLE legal_enquiries (
    id UUID PRIMARY KEY,
    -- 'terms' or 'privacy'. Takedowns are not a kind here: they have their own
    -- table because they have their own law.
    kind TEXT NOT NULL,
    submitter_name TEXT NOT NULL,
    -- How to answer them. Required, because an enquiry nobody can reply to
    -- cannot be resolved, and a privacy request in particular obliges an
    -- answer.
    submitter_contact TEXT NOT NULL,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    -- Set when the person happened to be signed in. Never required.
    submitted_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- Provenance of the answer, nullable because an open enquiry has none.
    handled_by UUID REFERENCES users(id) ON DELETE SET NULL,
    handled_at TIMESTAMP,
    resolution_note TEXT,

    CONSTRAINT legal_enquiries_kind_known
        CHECK (kind IN ('terms', 'privacy')),
    CONSTRAINT legal_enquiries_status_known
        CHECK (status IN ('open', 'acknowledged', 'closed')),
    -- A resolved enquiry has a resolver and a time; an open one has neither.
    -- Enforced here rather than in one code path, because the admin surface
    -- and any future automation both write this row.
    CONSTRAINT legal_enquiries_handled_together
        CHECK ((handled_by IS NULL) = (handled_at IS NULL))
);

-- The operator's question is always "what is open, of this kind, oldest
-- first" — an intake queue is worked front to back, unlike an audit trail.
CREATE INDEX legal_enquiries_kind_status_created_idx
    ON legal_enquiries(kind, status, created_at);
