-- Spec 025 (T004, FR-001/FR-003/FR-006/FR-024a): the core Ability entity.
--
-- Mirrors world_items (spec 013) with two deliberate differences:
--   * `updated_by` is present. world_items carries only `created_by`; spec 025
--     FR-027 requires both, per Constitution Principle III.
--   * `gm_only` is the visibility control. It is NOT a level in the ownership
--     block, because ActorPermissionLevel's lowest value (Viewer) is also its
--     default for a member with no row — the permission model structurally
--     cannot express "hidden". Mirrors scenes.hidden instead.
--
-- No uniqueness on `name` (FR-006, deliberate); the trigram index backs the
-- advisory "did you mean?" query and reuses the pg_trgm extension already
-- enabled by spec 013's own migration.
CREATE TABLE world_abilities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    classification VARCHAR(16) NOT NULL
        CHECK (classification IN ('spell', 'feat', 'power', 'talent')),
    gm_only BOOLEAN NOT NULL DEFAULT FALSE,
    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX world_abilities_world_id_idx ON world_abilities(world_id);
CREATE INDEX world_abilities_name_trgm_idx ON world_abilities USING GIN (name gin_trgm_ops);
