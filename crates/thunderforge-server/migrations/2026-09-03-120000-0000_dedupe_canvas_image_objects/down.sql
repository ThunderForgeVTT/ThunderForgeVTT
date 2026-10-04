-- Reversing this is only possible on a database that has not yet deduplicated
-- anything: restoring UNIQUE fails outright if two rows share a path, which is
-- the normal state after any upload of an image already stored.
--
-- Deliberately not "helpfully" deleting the duplicate rows to make the
-- constraint fit. Those rows are real assets belonging to real worlds, and a
-- down migration that silently unbackgrounds somebody's scenes to satisfy a
-- constraint is worse than one that refuses.
DROP INDEX IF EXISTS idx_canvas_image_assets_content_hash;
DROP INDEX IF EXISTS idx_canvas_image_assets_storage_path;

ALTER TABLE canvas_image_assets
  ADD CONSTRAINT canvas_image_assets_storage_path_key UNIQUE (storage_path);
