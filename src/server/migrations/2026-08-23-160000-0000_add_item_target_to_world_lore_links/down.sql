ALTER TABLE world_lore_links DROP CONSTRAINT world_lore_links_check;
ALTER TABLE world_lore_links ADD CONSTRAINT world_lore_links_check
    CHECK (target_lore_entry_id IS NULL OR target_actor_id IS NULL);

ALTER TABLE world_lore_links DROP CONSTRAINT world_lore_links_target_kind_check;
ALTER TABLE world_lore_links ADD CONSTRAINT world_lore_links_target_kind_check
    CHECK (target_kind IN ('lore_entry', 'actor', 'unresolved'));

DROP INDEX world_lore_links_target_item_id_idx;
ALTER TABLE world_lore_links DROP COLUMN target_item_id;
