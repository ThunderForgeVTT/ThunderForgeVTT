ALTER TABLE interaction_requests DROP COLUMN token_id;

DROP TRIGGER fog_masks_fill_entry_level_trigger ON fog_masks;
DROP TRIGGER interactives_fill_entry_level_trigger ON interactives;
DROP TRIGGER shapes_fill_entry_level_trigger ON shapes;
DROP TRIGGER light_sources_fill_entry_level_trigger ON light_sources;
DROP TRIGGER walls_fill_entry_level_trigger ON walls;
DROP TRIGGER tokens_fill_entry_level_trigger ON tokens;
DROP FUNCTION fill_entry_level();

-- One mask per scene again: the entry level's is the one that was there
-- before, so it is the one kept.
DELETE FROM fog_masks f
USING scene_levels l
WHERE l.level_id = f.level_id AND NOT l.is_entry;
ALTER TABLE fog_masks DROP CONSTRAINT fog_masks_scene_level_key;
ALTER TABLE fog_masks ADD CONSTRAINT fog_masks_scene_id_key UNIQUE (scene_id);

-- Everything on every level collapses back onto the one board a scene had.
-- Dropping the column drops its foreign key and its index with it.
ALTER TABLE fog_masks DROP COLUMN level_id;
ALTER TABLE interactives DROP COLUMN level_id;
ALTER TABLE shapes DROP COLUMN level_id;
ALTER TABLE light_sources DROP COLUMN level_id;
ALTER TABLE walls DROP COLUMN level_id;
ALTER TABLE tokens DROP COLUMN level_id;

DROP TRIGGER scenes_mirror_board_trigger ON scenes;
DROP FUNCTION mirror_scene_board_to_entry_level();
DROP TRIGGER scenes_create_entry_level_trigger ON scenes;
DROP FUNCTION create_entry_level();

DROP TABLE scene_levels;
