-- Spec 020 (FR-004, data-model.md "world_genie_shop_listings"): a
-- stocked item plus a configurable price — either a Session Resource
-- amount or an item-for-item barter, chosen per listing at creation.
-- "Stock" is not tracked here; it IS world_actor_inventory.quantity for
-- (actor_id, item_id) — the same NPC-inventory-as-stock primitive spec
-- 020's Research Summary confirmed needs zero schema changes.

CREATE TABLE world_genie_shop_listings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id UUID NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    item_id UUID NOT NULL REFERENCES world_items(id) ON DELETE CASCADE,
    price_kind TEXT NOT NULL CHECK (price_kind IN ('resource', 'item')),
    price_resource_type TEXT,
    price_resource_amount INTEGER CHECK (price_resource_amount IS NULL OR price_resource_amount > 0),
    price_item_id UUID REFERENCES world_items(id),
    price_item_quantity INTEGER CHECK (price_item_quantity IS NULL OR price_item_quantity > 0),
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (
        (price_kind = 'resource' AND price_resource_type IS NOT NULL AND price_resource_amount IS NOT NULL
            AND price_item_id IS NULL AND price_item_quantity IS NULL)
        OR
        (price_kind = 'item' AND price_item_id IS NOT NULL AND price_item_quantity IS NOT NULL
            AND price_resource_type IS NULL AND price_resource_amount IS NULL)
    )
);

CREATE INDEX world_genie_shop_listings_actor_id_idx ON world_genie_shop_listings(actor_id);
