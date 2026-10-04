-- Create tokens table for Phase 3.5
CREATE TABLE tokens (
  token_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  scene_id UUID NOT NULL REFERENCES scenes(scene_id) ON DELETE CASCADE,
  actor_id UUID,
  x FLOAT NOT NULL DEFAULT 0,
  y FLOAT NOT NULL DEFAULT 0,
  rotation FLOAT NOT NULL DEFAULT 0,
  scale FLOAT NOT NULL DEFAULT 1.0,
  metadata JSONB DEFAULT '{}'::jsonb,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  
  CONSTRAINT valid_scale CHECK (scale > 0),
  CONSTRAINT valid_coordinates CHECK (x >= 0 AND y >= 0)
);

-- Indexes for common queries
CREATE INDEX idx_tokens_scene_id ON tokens(scene_id);
CREATE INDEX idx_tokens_actor_id ON tokens(actor_id);
CREATE INDEX idx_tokens_created_at ON tokens(created_at DESC);
CREATE INDEX idx_tokens_updated_at ON tokens(updated_at DESC);
CREATE INDEX idx_tokens_scene_id_actor_id ON tokens(scene_id, actor_id);


