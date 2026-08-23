CREATE TABLE world_lore_entries (
    id UUID PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    -- FK to world_lore_revisions(id) added in the next migration once that
    -- table exists (the two tables reference each other).
    current_revision_id UUID,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (world_id, slug)
);

CREATE INDEX world_lore_entries_world_id_idx ON world_lore_entries(world_id);
