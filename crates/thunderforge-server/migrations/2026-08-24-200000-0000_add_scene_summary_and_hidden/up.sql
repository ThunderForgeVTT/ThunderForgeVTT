-- Spec 022 (Scene Management Overhaul, US1/US2): scenes gain a
-- GM-authored Markdown summary (rendered server-side, same pipeline as
-- lore entries), a player-facing hidden flag (hidden by default per
-- spec.md Clarifications), and a pointer at a generated preview/thumbnail
-- image, distinct from the full-resolution background image used in Play.
ALTER TABLE scenes
    ADD COLUMN summary_markdown TEXT,
    ADD COLUMN summary_rendered_html TEXT,
    ADD COLUMN hidden BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN preview_asset_id UUID;

-- A minimal, dedicated table for the generated preview rendition, mirroring
-- world_lore_image_assets' shape rather than repurposing
-- canvas_image_assets (whose `kind` is a native Postgres enum
-- Background/Pasted — extending it for a third, structurally different
-- purpose would be more invasive than a small new table). The actual
-- image bytes live in the same RustFS-backed object storage every other
-- asset table already uses, addressed by this row's `id`.
CREATE TABLE scene_preview_images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scene_id UUID NOT NULL REFERENCES scenes(scene_id) ON DELETE CASCADE,
    byte_size BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT now()
);

CREATE INDEX scene_preview_images_scene_id_idx ON scene_preview_images(scene_id);

ALTER TABLE scenes
    ADD CONSTRAINT scenes_preview_asset_id_fkey
    FOREIGN KEY (preview_asset_id) REFERENCES scene_preview_images(id) ON DELETE SET NULL;
