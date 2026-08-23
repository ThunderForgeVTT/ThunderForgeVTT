CREATE TABLE world_actor_inventory (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id UUID NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    item_id UUID REFERENCES world_items(id) ON DELETE SET NULL,
    item_name_snapshot TEXT NOT NULL,
    quantity INTEGER NOT NULL CHECK (quantity >= 0),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (actor_id, item_id)
);

CREATE INDEX world_actor_inventory_actor_id_idx ON world_actor_inventory(actor_id);
CREATE INDEX world_actor_inventory_item_id_idx ON world_actor_inventory(item_id);
