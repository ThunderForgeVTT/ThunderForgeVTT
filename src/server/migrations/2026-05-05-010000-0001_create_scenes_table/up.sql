-- Create scenes table for Phase 3.5
CREATE TABLE scenes (
  scene_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  description TEXT,
  type TEXT NOT NULL DEFAULT 'battlemap' CHECK (type IN ('battlemap', 'region', 'board', 'custom')),
  grid_size INT NOT NULL DEFAULT 5,
  grid_type TEXT NOT NULL DEFAULT 'square' CHECK (grid_type IN ('square', 'hex')),
  width INT NOT NULL DEFAULT 100,
  height INT NOT NULL DEFAULT 100,
  metadata JSONB DEFAULT '{}'::jsonb,
  owner_id UUID NOT NULL REFERENCES users(id),
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  
  CONSTRAINT unique_scene_name_per_world UNIQUE(world_id, name),
  CONSTRAINT valid_dimensions CHECK (width > 0 AND height > 0),
  CONSTRAINT valid_grid_size CHECK (grid_size > 0)
);

-- Indexes for common queries
CREATE INDEX idx_scenes_world_id ON scenes(world_id);
CREATE INDEX idx_scenes_owner_id ON scenes(owner_id);
CREATE INDEX idx_scenes_created_at ON scenes(created_at DESC);
CREATE INDEX idx_scenes_updated_at ON scenes(updated_at DESC);
CREATE INDEX idx_scenes_world_id_name ON scenes(world_id, name);

