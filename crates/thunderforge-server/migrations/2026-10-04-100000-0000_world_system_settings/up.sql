-- Spec 067 Story 1: what a table chooses about how it plays its ruleset.
--
-- One shared table, and that is the decision. ADR-108 recorded that two
-- packs storing per-world settings in tables of their own had met ADR-063's
-- threshold for a shared surface; this is that surface. A pack declares its
-- settings in its manifest (`pack_system_spec::settings`) and writes no
-- migration for one.
--
-- It is not a key/value bucket, for the reason `instance_settings` is not
-- (ADR-091): a row means something only while the system's manifest declares
-- its key. A row whose key nothing declares is INERT — it does not resolve
-- and it is never deleted, so a world that changes system and changes back,
-- or a pack that drops a setting and restores it, finds its answers as they
-- were left.
--
-- A missing row is the declared default. Nothing is seeded, so a world that
-- has never opened its settings and a world that predates this table are the
-- same world.

CREATE TABLE world_system_settings (
    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    -- The system whose manifest declares `key`. Part of the key so that two
    -- systems may each have a setting called the same thing, and so that a
    -- world's answers for one system survive a spell on another.
    system_id TEXT NOT NULL,
    key TEXT NOT NULL,
    -- The value as the declaration typed it: a JSON boolean, number or
    -- string. Validated against the declaration on write and again on read.
    value JSONB NOT NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (world_id, system_id, key)
);

-- Every change, appended and never rewritten: who changed which rule of the
-- game, from what, to what, and when. Rows leave only with their world.
CREATE TABLE world_system_setting_changes (
    id BIGSERIAL PRIMARY KEY,
    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    system_id TEXT NOT NULL,
    key TEXT NOT NULL,
    -- NULL when the setting had never been set: the change was from the
    -- declared default, whatever that was at the time.
    old_value JSONB,
    new_value JSONB NOT NULL,
    changed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    changed_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX world_system_setting_changes_world_idx
    ON world_system_setting_changes (world_id, changed_at);
