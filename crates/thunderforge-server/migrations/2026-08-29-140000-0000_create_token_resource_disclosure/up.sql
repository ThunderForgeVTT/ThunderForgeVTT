-- What each token discloses about each of its resources, to everyone who is
-- not running the world.
--
-- Sparse on purpose. Absence means the world default, so a table where nobody
-- has overridden anything stores no rows at all — which is the common case.
--
-- Keyed per *token* rather than per actor: two tokens of the same creature can
-- legitimately differ, and the Game Master sets this on the one standing in
-- front of the players.
CREATE TABLE token_resource_disclosure (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token_id UUID NOT NULL REFERENCES tokens(token_id) ON DELETE CASCADE,

    -- The game system's ResourceDefinition.id. Deliberately not a foreign key:
    -- definitions live in system packages rather than in this database, and a
    -- definition may be withdrawn and later reinstated. A row for a resource
    -- nobody currently declares is simply not displayed.
    resource_id VARCHAR NOT NULL,

    -- 'visible' | 'greyed' | 'percentage' | 'chunked'
    state VARCHAR NOT NULL,

    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),

    CONSTRAINT token_resource_disclosure_unique UNIQUE (token_id, resource_id)
);

-- The read is always "everything disclosed for these tokens", never a scan by
-- resource, so the token is the leading column.
CREATE INDEX token_resource_disclosure_token_idx
    ON token_resource_disclosure (token_id);
