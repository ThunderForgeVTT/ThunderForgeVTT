-- Recreates the table's last shape (the 2026-05-02 create plus the 2026-05-04
-- ownership columns). The rows are not restored.
CREATE TABLE world_tokens (
    id TEXT NOT NULL PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worlds (id) ON DELETE CASCADE,
    x FLOAT NOT NULL DEFAULT 0.0,
    y FLOAT NOT NULL DEFAULT 0.0,
    z FLOAT NOT NULL DEFAULT 0.0,
    label TEXT,
    health INTEGER,
    max_health INTEGER,
    schema_version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by UUID NOT NULL,
    updated_by UUID NOT NULL,
    CONSTRAINT world_tokens_created_by_fkey FOREIGN KEY (created_by) REFERENCES users (id),
    CONSTRAINT world_tokens_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES users (id)
);

CREATE INDEX idx_world_tokens_world_id ON world_tokens (world_id);
CREATE INDEX idx_world_tokens_created_at ON world_tokens (created_at);
CREATE INDEX idx_world_tokens_created_by ON world_tokens (created_by);
