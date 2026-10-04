-- Phase: Native canvas authoring - shapes (FR-007, FR-008), full tldraw replacement
CREATE TABLE shapes (
  shape_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  scene_id UUID NOT NULL REFERENCES scenes(scene_id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('stroke', 'rect', 'ellipse', 'line', 'text')),
  geometry JSONB NOT NULL,
  text TEXT,
  style JSONB,
  visible_to_players BOOLEAN NOT NULL DEFAULT FALSE,
  metadata JSONB,
  created_by UUID NOT NULL REFERENCES users(id),
  updated_by UUID NOT NULL REFERENCES users(id),
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_shapes_scene_id ON shapes(scene_id);
