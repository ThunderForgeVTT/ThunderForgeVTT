-- Phase 6: Walls (vision-blocking line segments per scene)
CREATE TABLE walls (
  wall_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  scene_id UUID NOT NULL REFERENCES scenes(scene_id) ON DELETE CASCADE,
  x1 DOUBLE PRECISION NOT NULL,
  y1 DOUBLE PRECISION NOT NULL,
  x2 DOUBLE PRECISION NOT NULL,
  y2 DOUBLE PRECISION NOT NULL,
  blocks_vision BOOLEAN NOT NULL DEFAULT TRUE,
  blocks_movement BOOLEAN NOT NULL DEFAULT FALSE,
  metadata JSONB,
  created_by UUID NOT NULL REFERENCES users(id),
  updated_by UUID NOT NULL REFERENCES users(id),
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_walls_scene_id ON walls(scene_id);
