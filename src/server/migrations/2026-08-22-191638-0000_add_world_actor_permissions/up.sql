CREATE TABLE world_actor_permissions (
    id UUID PRIMARY KEY,
    actor_id UUID NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    level VARCHAR(16) NOT NULL CHECK (level IN ('Viewer', 'Editor', 'Owner')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (actor_id, user_id)
);

CREATE INDEX world_actor_permissions_actor_id_idx ON world_actor_permissions(actor_id);
CREATE INDEX world_actor_permissions_user_id_idx ON world_actor_permissions(user_id);
