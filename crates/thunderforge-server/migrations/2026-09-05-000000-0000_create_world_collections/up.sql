-- Spec 026 (T005), governed by ADR-069 (the DMCA determination) and ADR-070
-- (the anonymous read path).
--
-- Note what is deliberately ABSENT, as the spec-025 share migrations do: any
-- index or query shape that would let collections or shares be listed across
-- worlds or across users. FR-020's no-enumeration property is guaranteed
-- structurally, by there being nothing to enumerate with, and ADR-069's
-- determination is conditional on it. Do not add a "list all collections" path.

CREATE TABLE world_collections (
    id UUID PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Serves an owner listing their OWN world's collections, which FR-020 permits
-- ("beyond a user's own"). It does not serve, and nothing may add, a lookup
-- that reaches collections across worlds or by anyone but their owner.
CREATE INDEX world_collections_world_id_idx ON world_collections(world_id);

-- member_id carries NO foreign key, and that is the decision rather than an
-- omission.
--
-- A polymorphic column cannot carry one, and the alternative — five typed
-- membership tables — would be five near-identical tables and five places to
-- forget a type. More importantly, a cascading FK would silently delete the
-- membership row when its artifact is deleted, and spec 026 requires the
-- opposite: a member deleted from its world must not make the collection
-- unopenable. So the row outlives its artifact and the read path resolves each
-- member, treating what no longer resolves as withheld.
--
-- ADR-050 weighed the same tradeoff for the four permission tables and resolved
-- it the other way, because there cascade-on-delete was the DESIRED behaviour.
-- Here it is the failure.
CREATE TABLE world_collection_members (
    id UUID PRIMARY KEY,
    collection_id UUID NOT NULL REFERENCES world_collections(id) ON DELETE CASCADE,
    member_type VARCHAR(32) NOT NULL,
    member_id UUID NOT NULL,
    sort_order INT NOT NULL DEFAULT 0,
    added_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT world_collection_members_unique UNIQUE (collection_id, member_type, member_id)
);

CREATE INDEX world_collection_members_collection_id_idx
    ON world_collection_members(collection_id);

-- There is deliberately NO `disabled` and NO `restricted` column here.
--
-- Moderation status is asked at read and copy time via
-- moderation::effective_status, which performs lazy auto-restoration: a
-- counter-notice whose waiting period has elapsed restores when read. A cached
-- flag would be stale in BOTH directions — serving a taken-down artifact, or
-- withholding one that FR-025 says should have returned. The same argument
-- covers restriction: FR-001b requires an artifact that BECOMES restricted to
-- be withheld from that point, which a value captured at add time cannot say.

-- share_code is derived from a v4 UUID, never v7: spec 005 found a real
-- collision where codes taken from a v7 UUID's leading hex characters collided
-- within the same millisecond, because v7 front-loads a timestamp.
CREATE TABLE world_collection_shares (
    id UUID PRIMARY KEY,
    collection_id UUID NOT NULL REFERENCES world_collections(id) ON DELETE CASCADE,
    share_code VARCHAR(32) NOT NULL UNIQUE,
    created_by UUID NOT NULL REFERENCES users(id),
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX world_collection_shares_collection_id_idx
    ON world_collection_shares(collection_id);
