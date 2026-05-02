-- Create world_tokens table for game token/piece tracking
CREATE TABLE world_tokens (
    id TEXT NOT NULL PRIMARY KEY,
    world_id UUID NOT NULL,
    x FLOAT NOT NULL DEFAULT 0.0,
    y FLOAT NOT NULL DEFAULT 0.0,
    z FLOAT NOT NULL DEFAULT 0.0,
    label TEXT,
    health INTEGER,
    max_health INTEGER,
    schema_version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE
);

CREATE INDEX idx_world_tokens_world_id ON world_tokens(world_id);
CREATE INDEX idx_world_tokens_created_at ON world_tokens(created_at);
