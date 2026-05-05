-- Create fog_masks table for Phase 3.5
CREATE TABLE fog_masks (
  fog_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  scene_id UUID NOT NULL UNIQUE REFERENCES scenes(scene_id) ON DELETE CASCADE,
  bitmap_data BYTEA NOT NULL,
  version INT NOT NULL DEFAULT 1,
  width INT NOT NULL,
  height INT NOT NULL,
  updated_by UUID NOT NULL REFERENCES users(id),
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  
  CONSTRAINT valid_dimensions CHECK (width > 0 AND height > 0),
  CONSTRAINT valid_version CHECK (version > 0)
);

-- Indexes for common queries
CREATE INDEX idx_fog_masks_scene_id ON fog_masks(scene_id);
CREATE INDEX idx_fog_masks_updated_by ON fog_masks(updated_by);
CREATE INDEX idx_fog_masks_created_at ON fog_masks(created_at DESC);
CREATE INDEX idx_fog_masks_updated_at ON fog_masks(updated_at DESC);

