CREATE TABLE world_lore_links (
    id UUID PRIMARY KEY,
    source_lore_entry_id UUID NOT NULL REFERENCES world_lore_entries(id) ON DELETE CASCADE,
    raw_title TEXT NOT NULL,
    target_kind VARCHAR(16) NOT NULL CHECK (target_kind IN ('lore_entry', 'actor', 'unresolved')),
    target_lore_entry_id UUID REFERENCES world_lore_entries(id) ON DELETE SET NULL,
    target_actor_id UUID REFERENCES world_actors(id) ON DELETE SET NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- At most one target column may be set. This is intentionally looser
    -- than "target_kind must match the non-null column" (data-model.md's
    -- literal wording): a stricter check tied to target_kind would be
    -- re-evaluated (and fail) when ON DELETE SET NULL fires on a deleted
    -- target, which would block the very deletion FR-020 requires never
    -- be blocked. target_kind is authoritative only at insert time; a row
    -- whose target FK has since gone NULL is treated as unresolved by
    -- every read path regardless of its stored target_kind label
    -- (data-model.md's "Validation rules" note).
    CHECK (target_lore_entry_id IS NULL OR target_actor_id IS NULL)
);

CREATE INDEX world_lore_links_source_lore_entry_id_idx ON world_lore_links(source_lore_entry_id);
CREATE INDEX world_lore_links_target_lore_entry_id_idx ON world_lore_links(target_lore_entry_id);
CREATE INDEX world_lore_links_target_actor_id_idx ON world_lore_links(target_actor_id);
