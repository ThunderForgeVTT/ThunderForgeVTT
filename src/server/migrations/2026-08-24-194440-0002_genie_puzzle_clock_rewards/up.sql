-- Spec 020 (FR-006, data-model.md "world_genie_puzzle_clock_rewards"):
-- any number of configured reward entries per Puzzle Clock, each tied to
-- a trigger segment, a resource-or-item payout, and a recipient rule.
-- granted_at guarantees "exactly once" (research.md R4) — a row is
-- granted only once, ever, guarded by granted_at IS NULL.

CREATE TABLE world_genie_puzzle_clock_rewards (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    clock_id UUID NOT NULL REFERENCES world_genie_puzzle_clocks(id) ON DELETE CASCADE,
    trigger_segment INTEGER NOT NULL CHECK (trigger_segment > 0),
    reward_resource_type TEXT,
    reward_resource_amount INTEGER CHECK (reward_resource_amount IS NULL OR reward_resource_amount > 0),
    reward_item_id UUID REFERENCES world_items(id),
    reward_item_quantity INTEGER CHECK (reward_item_quantity IS NULL OR reward_item_quantity > 0),
    recipient_mode TEXT NOT NULL CHECK (recipient_mode IN ('triggering_actor', 'whole_party')),
    granted_at TIMESTAMP,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (
        (reward_resource_type IS NOT NULL AND reward_resource_amount IS NOT NULL
            AND reward_item_id IS NULL AND reward_item_quantity IS NULL)
        OR
        (reward_item_id IS NOT NULL AND reward_item_quantity IS NOT NULL
            AND reward_resource_type IS NULL AND reward_resource_amount IS NULL)
    )
);

CREATE INDEX world_genie_puzzle_clock_rewards_clock_id_idx ON world_genie_puzzle_clock_rewards(clock_id);
