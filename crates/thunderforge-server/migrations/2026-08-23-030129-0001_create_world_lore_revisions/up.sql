CREATE TABLE world_lore_revisions (
    id UUID PRIMARY KEY,
    lore_entry_id UUID NOT NULL REFERENCES world_lore_entries(id) ON DELETE CASCADE,
    content_markdown TEXT NOT NULL,
    author_id UUID NOT NULL REFERENCES users(id),
    restored_from_revision_id UUID REFERENCES world_lore_revisions(id) ON DELETE SET NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX world_lore_revisions_lore_entry_id_idx ON world_lore_revisions(lore_entry_id);

-- Now that world_lore_revisions exists, wire up the forward reference
-- from world_lore_entries.current_revision_id (data-model.md: the two
-- tables reference each other, so this FK could not be added in the
-- entries migration).
ALTER TABLE world_lore_entries
    ADD CONSTRAINT world_lore_entries_current_revision_id_fkey
    FOREIGN KEY (current_revision_id) REFERENCES world_lore_revisions(id) ON DELETE SET NULL;
