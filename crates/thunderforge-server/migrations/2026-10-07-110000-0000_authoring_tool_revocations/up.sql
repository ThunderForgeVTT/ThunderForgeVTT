-- Spec 082: players select and draw by default; a Game Master can take
-- either tool away from one player.
--
-- The defaults live in code (`PLAYER_DEFAULT_TOOLS`), so every world has
-- them on the next request with no backfill. What a Game Master takes away
-- is recorded here, as the opposite fact of a grant: a grant row always
-- widens what a player holds, a revocation row always narrows it.
--
-- Keyed on the membership, like the grants, so a player who leaves takes
-- their revocations with them and rejoins with the defaults.
--
-- `revoked_by` is SET NULL, unlike the grants' provenance, so deleting the
-- account of the Game Master who wrote it never fails on this row.
CREATE TABLE world_authoring_tool_revocations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    world_member_id UUID NOT NULL REFERENCES world_members(id) ON DELETE CASCADE,
    tool VARCHAR(32) NOT NULL CHECK (tool IN ('select', 'shapes')),
    revoked_by UUID REFERENCES users(id) ON DELETE SET NULL,
    revoked_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (world_member_id, tool)
);

-- A grant never names a default tool: holding it is the default, and a
-- second way to say "held" is a second spelling of one permission.
DELETE FROM world_authoring_tool_grants WHERE tool IN ('select', 'shapes');
ALTER TABLE world_authoring_tool_grants
    ADD CONSTRAINT world_authoring_tool_grants_not_a_default
        CHECK (tool NOT IN ('select', 'shapes'));
