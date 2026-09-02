-- Spec 031 (T032b, FR-046): the tools one player has been granted in one
-- world. The store `auth::authoring_tools::granted_authoring_tools` reads.
--
-- Keyed on `world_member_id`, not on a `(world_id, user_id)` pair, which is
-- the one real decision here. A grant is a fact about somebody's membership
-- of a world, and membership already has a row with a primary key. Keying on
-- it buys the whole of FR-046's cleanup for free: removing a member deletes
-- their `world_members` row, and this table's ON DELETE CASCADE takes the
-- grants with it. The `(world_id, user_id)` shape cannot cascade — there is
-- no row for the pair — so every grant would have to be deleted by hand in
-- `remove_member_impl`. ADR-050 exists because that hand-written cleanup was
-- written once per content type and then not written for the fifth; adding a
-- sixth block to the same list is the mistake, not the fix.
--
-- `world_actor_claims` keys on `world_members(id)` for exactly this reason
-- and is the precedent followed here.
--
-- `tool` is open text with no CHECK, on ADR-054's reasoning and matching
-- `world_actor_images.role`: the vocabulary is `AUTHORING_TOOLS` in the
-- server, `GM_TOOL_IDS` in the web app and `AuthoringMode::from_tool_id` in
-- the engine, and a fixed set in SQL would make renaming a tool a migration.
-- A row naming a tool no build knows is refused at the resolver and ignored
-- by the rail — it fails closed, which a CHECK would not improve.
--
-- Provenance follows world_actor_images (spec 025/031), NOT NULL against
-- users(id), per Constitution Principle III. Both columns are meaningful
-- here and are not the same person as `world_member_id`: the subject of a
-- grant never writes it, since only a Game Master may (FR-046). `created_by`
-- is therefore "which GM handed this out", which is the question an argument
-- about a player's tools actually asks.
CREATE TABLE world_authoring_tool_grants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    world_member_id UUID NOT NULL REFERENCES world_members(id) ON DELETE CASCADE,
    tool VARCHAR NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- A grant is held or it is not; a second row for the same tool would be
    -- a second copy of one yes/no. Granting is written as an upsert against
    -- this constraint, so a double click grants once.
    UNIQUE (world_member_id, tool)
);

-- No separate index on `world_member_id`. Every read is "what has this member
-- been granted", and the unique constraint's index leads with that column, so
-- a second index would be the same index.
