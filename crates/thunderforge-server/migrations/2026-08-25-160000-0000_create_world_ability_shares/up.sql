-- Spec 025 (T083, FR-032/FR-036/FR-037), governed by ADR-049.
--
-- Note what is deliberately ABSENT: any index or query shape that would let
-- shares be listed by world, by user, or in aggregate. FR-037's
-- no-enumeration property is guaranteed structurally, by there being nothing
-- to enumerate with — and it is one of the six invariants ADR-049's DMCA
-- determination is conditional on. Do not add a "list shares" path here.
--
-- share_code is derived from a v4 UUID, never v7: spec 005 found a real
-- collision bug where codes taken from a v7 UUID's leading hex characters
-- collided within the same millisecond, because v7 front-loads a timestamp.
CREATE TABLE world_ability_shares (
    id UUID PRIMARY KEY,
    ability_id UUID NOT NULL REFERENCES world_abilities(id) ON DELETE CASCADE,
    share_code VARCHAR(32) NOT NULL UNIQUE,
    created_by UUID NOT NULL REFERENCES users(id),
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX world_ability_shares_ability_id_idx ON world_ability_shares(ability_id);
