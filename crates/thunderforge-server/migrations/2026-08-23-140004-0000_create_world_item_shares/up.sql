CREATE TABLE world_item_shares (
    id UUID PRIMARY KEY,
    item_id UUID NOT NULL REFERENCES world_items(id) ON DELETE CASCADE,
    share_code VARCHAR(32) NOT NULL UNIQUE,
    created_by UUID NOT NULL REFERENCES users(id),
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX world_item_shares_item_id_idx ON world_item_shares(item_id);
