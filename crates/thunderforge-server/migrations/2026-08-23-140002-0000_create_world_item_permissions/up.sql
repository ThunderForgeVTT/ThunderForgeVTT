CREATE TABLE world_item_permissions (
    id UUID PRIMARY KEY,
    item_id UUID NOT NULL REFERENCES world_items(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    level VARCHAR(16) NOT NULL CHECK (level IN ('Viewer', 'Editor', 'Owner')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (item_id, user_id)
);

CREATE INDEX world_item_permissions_item_id_idx ON world_item_permissions(item_id);
CREATE INDEX world_item_permissions_user_id_idx ON world_item_permissions(user_id);
