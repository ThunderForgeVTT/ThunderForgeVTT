-- Many asset rows, one stored object.
--
-- `storage_path` has been UNIQUE since spec 002, when every upload wrote its
-- own object and one row per object was simply true. Deduplication makes it
-- false on purpose: an image already stored is referenced again rather than
-- written again, so several rows — in different worlds, owned by different
-- people — legitimately name one path.
--
-- Each row keeps its own `asset_id`, `world_id`, `scene_id` and owner, and
-- `canvas_assets_serve` authorises against the row it looked up. Sharing the
-- bytes shares nothing about who may see them.
--
-- Measured on the development database on 2026-09-03 before this landed:
-- 4,387 asset rows holding 61 distinct images, 2,695 MB stored for 116 MB of
-- content.
ALTER TABLE canvas_image_assets
  DROP CONSTRAINT IF EXISTS canvas_image_assets_storage_path_key;

-- Non-unique, because the column is no longer unique — but still indexed, and
-- for a specific future need: deleting an object safely requires asking "does
-- any other row name this path", and that question must be cheap. See the
-- reference-counting note in `storage/dedupe.rs`.
CREATE INDEX IF NOT EXISTS idx_canvas_image_assets_storage_path
  ON canvas_image_assets(storage_path);

-- The lookup deduplication actually performs, on every upload and every map
-- import. Without it each one is a sequential scan of the asset table.
CREATE INDEX IF NOT EXISTS idx_canvas_image_assets_content_hash
  ON canvas_image_assets(content_hash)
  WHERE content_hash IS NOT NULL;
