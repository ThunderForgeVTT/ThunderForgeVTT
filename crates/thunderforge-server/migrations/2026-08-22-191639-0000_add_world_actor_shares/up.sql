CREATE TABLE world_actor_shares (
    id UUID PRIMARY KEY,
    actor_id UUID NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    share_code VARCHAR(32) NOT NULL UNIQUE,
    created_by UUID NOT NULL REFERENCES users(id),
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX world_actor_shares_actor_id_idx ON world_actor_shares(actor_id);
