-- Spec 088 (FR-029): a background made from one of the base maps records the
-- map's id (its `maps.json` id, e.g. `grassy-path-ambush`), so the scene can
-- show the map's credit. No foreign key: the base maps are files, not rows.
ALTER TABLE canvas_image_assets ADD COLUMN base_map_id TEXT NULL;
