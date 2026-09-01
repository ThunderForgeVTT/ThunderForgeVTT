DROP TABLE world_lore_tags;

DROP INDEX world_lore_entries_parent_id_idx;

ALTER TABLE world_lore_entries
    DROP CONSTRAINT world_lore_entries_parent_id_fkey;

ALTER TABLE world_lore_entries
    DROP COLUMN parent_id;
