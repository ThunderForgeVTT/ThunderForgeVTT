CREATE TABLE world_item_effects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    item_id UUID NOT NULL REFERENCES world_items(id) ON DELETE CASCADE,
    effect_type VARCHAR(16) NOT NULL CHECK (effect_type IN ('heal', 'damage', 'modifier', 'attack_roll')),
    formula TEXT NOT NULL,
    target TEXT NOT NULL,
    trigger_kind VARCHAR(16) CHECK (trigger_kind IS NULL OR trigger_kind IN ('on_use', 'passive')),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX world_item_effects_item_id_idx ON world_item_effects(item_id);
