ALTER TABLE scenes DROP CONSTRAINT scenes_preview_asset_id_fkey;
DROP TABLE scene_preview_images;
ALTER TABLE scenes
    DROP COLUMN summary_markdown,
    DROP COLUMN summary_rendered_html,
    DROP COLUMN hidden,
    DROP COLUMN preview_asset_id;
