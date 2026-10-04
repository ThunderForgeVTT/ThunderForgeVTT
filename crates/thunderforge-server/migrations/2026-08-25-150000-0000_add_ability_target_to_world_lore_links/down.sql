ALTER TABLE world_lore_links DROP CONSTRAINT world_lore_links_check;
ALTER TABLE world_lore_links ADD CONSTRAINT world_lore_links_check
    CHECK (
        (CASE WHEN target_lore_entry_id IS NOT NULL THEN 1 ELSE 0 END) +
        (CASE WHEN target_actor_id      IS NOT NULL THEN 1 ELSE 0 END) +
        (CASE WHEN target_item_id       IS NOT NULL THEN 1 ELSE 0 END) <= 1
    );

ALTER TABLE world_lore_links DROP CONSTRAINT world_lore_links_target_kind_check;
ALTER TABLE world_lore_links ADD CONSTRAINT world_lore_links_target_kind_check
    CHECK (target_kind IN ('lore_entry', 'actor', 'item', 'unresolved'));

DROP INDEX world_lore_links_target_ability_id_idx;
ALTER TABLE world_lore_links DROP COLUMN target_ability_id;
