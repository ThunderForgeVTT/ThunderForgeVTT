ALTER TABLE world_actors ADD COLUMN available_for_claim BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE worlds ADD COLUMN allow_player_created_actors BOOLEAN NOT NULL DEFAULT false;

CREATE TABLE world_actor_claims (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id UUID NOT NULL UNIQUE REFERENCES world_actors(id) ON DELETE CASCADE,
    world_member_id UUID NOT NULL UNIQUE REFERENCES world_members(id) ON DELETE CASCADE,
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
