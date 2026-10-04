-- Spec 067 Story 4: the conditions a character is under.
--
-- On the actor, and that is the decision: a condition follows the character
-- from scene to scene and is drawn on every token of theirs. A token with no
-- actor carries none.
--
-- A row means something only while the manifest of the world's system
-- declares its `condition_id` (`pack_system_spec::conditions`). A row whose
-- id nothing declares is INERT — it is never sent to a client and never
-- deleted, as a stored setting nothing declares is not
-- (`world_system_settings`).

CREATE TABLE world_actor_conditions (
    actor_id UUID NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    condition_id TEXT NOT NULL,
    applied_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (actor_id, condition_id)
);
