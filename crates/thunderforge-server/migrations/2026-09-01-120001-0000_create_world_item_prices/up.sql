-- Spec 031 (T012, US8/FR-037): the Game Master's note of what an item costs.
--
-- Presentational only (ADR-058). It participates in no transaction. A game
-- system with its own economy — world_genie_shop_listings is the existing
-- example, keyed per *vendor* — continues to own trade, and may display,
-- ignore or override this value. The generic layer must not reimplement
-- vendor pricing.
--
-- Its own table rather than columns on world_items, for the same reason Genie
-- put its economy in its own table: price is not a property every item in
-- every ruleset has.
--
-- `currency_label` is free text; this layer names no currency system.
-- `is_suggested` distinguishes "role-play from about this" from a set price.
--
-- Provenance follows world_abilities (spec 025): created_by and updated_by,
-- NOT NULL against users(id), per Constitution Principle III.
CREATE TABLE world_item_prices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Unique, not just a foreign key: at most one price per item. This is the
    -- GM's note, not a price list.
    item_id UUID NOT NULL UNIQUE REFERENCES world_items(id) ON DELETE CASCADE,
    amount INTEGER NOT NULL,
    currency_label TEXT,
    is_suggested BOOLEAN NOT NULL DEFAULT FALSE,
    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
