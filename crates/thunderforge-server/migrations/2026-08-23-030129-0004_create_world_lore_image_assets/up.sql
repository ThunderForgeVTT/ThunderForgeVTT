CREATE TABLE world_lore_image_assets (
    id UUID PRIMARY KEY,
    lore_entry_id UUID NOT NULL REFERENCES world_lore_entries(id) ON DELETE CASCADE,
    uploaded_by UUID NOT NULL REFERENCES users(id),
    original_filename TEXT,
    content_type TEXT NOT NULL,
    byte_size BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX world_lore_image_assets_lore_entry_id_idx ON world_lore_image_assets(lore_entry_id);
