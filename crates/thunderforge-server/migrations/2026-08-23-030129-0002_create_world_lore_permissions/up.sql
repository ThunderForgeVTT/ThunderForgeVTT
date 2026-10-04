CREATE TABLE world_lore_permissions (
    id UUID PRIMARY KEY,
    lore_entry_id UUID NOT NULL REFERENCES world_lore_entries(id) ON DELETE CASCADE,
    world_member_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    level VARCHAR(16) NOT NULL CHECK (level IN ('Viewer', 'Editor', 'Owner')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (lore_entry_id, world_member_user_id)
);

CREATE INDEX world_lore_permissions_lore_entry_id_idx ON world_lore_permissions(lore_entry_id);
CREATE INDEX world_lore_permissions_world_member_user_id_idx ON world_lore_permissions(world_member_user_id);
