-- Spec 013 (US3): Items become a third valid `[[...]]` link target from
-- lore, alongside lore entries and actors. See contracts/item-lore-links.md.

ALTER TABLE world_lore_links
    ADD COLUMN target_item_id UUID REFERENCES world_items(id) ON DELETE SET NULL;

CREATE INDEX world_lore_links_target_item_id_idx ON world_lore_links(target_item_id);

-- Extend target_kind to allow 'item'.
ALTER TABLE world_lore_links DROP CONSTRAINT world_lore_links_target_kind_check;
ALTER TABLE world_lore_links ADD CONSTRAINT world_lore_links_target_kind_check
    CHECK (target_kind IN ('lore_entry', 'actor', 'item', 'unresolved'));

-- Extend "at most one target column set" from 2-way to 3-way (same
-- insert-time-only semantics as the original — see the up.sql comment on
-- create_world_lore_links for why this can't be re-checked on later
-- ON DELETE SET NULL updates).
ALTER TABLE world_lore_links DROP CONSTRAINT world_lore_links_check;
ALTER TABLE world_lore_links ADD CONSTRAINT world_lore_links_check
    CHECK (
        (CASE WHEN target_lore_entry_id IS NOT NULL THEN 1 ELSE 0 END) +
        (CASE WHEN target_actor_id IS NOT NULL THEN 1 ELSE 0 END) +
        (CASE WHEN target_item_id IS NOT NULL THEN 1 ELSE 0 END) <= 1
    );
