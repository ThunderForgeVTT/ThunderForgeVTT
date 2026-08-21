-- Phase: Native canvas authoring - light sources (FR-004..FR-006, FR-027)
CREATE TABLE light_sources (
  light_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  scene_id UUID NOT NULL REFERENCES scenes(scene_id) ON DELETE CASCADE,
  x DOUBLE PRECISION NOT NULL,
  y DOUBLE PRECISION NOT NULL,
  radius DOUBLE PRECISION NOT NULL CHECK (radius > 0),
  intensity DOUBLE PRECISION NOT NULL DEFAULT 1.0 CHECK (intensity >= 0),
  color TEXT,
  attached_token_id UUID REFERENCES tokens(token_id) ON DELETE SET NULL,
  casts_shadows BOOLEAN NOT NULL DEFAULT TRUE,
  metadata JSONB,
  created_by UUID NOT NULL REFERENCES users(id),
  updated_by UUID NOT NULL REFERENCES users(id),
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_light_sources_scene_id ON light_sources(scene_id);
