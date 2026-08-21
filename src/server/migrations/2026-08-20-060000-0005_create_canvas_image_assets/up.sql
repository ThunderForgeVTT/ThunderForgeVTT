-- Spec 002: canvas image assets (paste-to-canvas + migrated map-import
-- backgrounds), backed by RustFS object storage (FR-011..FR-019).
CREATE TYPE "CanvasImageAssetKind" AS ENUM ('Background', 'Pasted');

CREATE TABLE canvas_image_assets (
  asset_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
  scene_id UUID REFERENCES scenes(scene_id) ON DELETE CASCADE,
  owner_user_id UUID NOT NULL REFERENCES users(id),
  storage_path TEXT NOT NULL UNIQUE,
  original_format TEXT NOT NULL,
  width_px INTEGER NOT NULL,
  height_px INTEGER NOT NULL,
  byte_size BIGINT NOT NULL,
  kind "CanvasImageAssetKind" NOT NULL,
  created_by UUID NOT NULL REFERENCES users(id),
  updated_by UUID NOT NULL REFERENCES users(id),
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_canvas_image_assets_world_id ON canvas_image_assets(world_id);
CREATE INDEX idx_canvas_image_assets_scene_id ON canvas_image_assets(scene_id);
