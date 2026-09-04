-- Spec 033 User Story 4 (FR-018 to FR-023): types declare what they bind to
-- and how they are graded.

-- The value on a type's declared grade — 5e's spell Level, another system's
-- Rank or Circle. One shape, many words.
--
-- NULL means the ability's type declares no grade, which is the common case
-- and must stay the cheap one.
--
-- Deliberately **unconstrained**. FR-023 says a value already recorded that
-- falls outside a *newly* declared range is retained and displayed, never
-- clamped or discarded — a system narrowing its range does not get to edit
-- content authored under the old one. The range is checked at authoring time
-- against the vocabulary in force, which is the only place that knows it.
ALTER TABLE world_abilities ADD COLUMN grade INTEGER;

-- The item counterpart of world_actor_abilities, mirroring it deliberately.
--
--   * `ability_id` is NULLABLE with ON DELETE SET NULL, so deleting an ability
--     never blocks on items carrying it; the row survives as a tombstone.
--   * `ability_name_snapshot` keeps a tombstoned row identifiable.
--   * UNIQUE (item_id, ability_id) — an item carries an ability once.
--
-- This table belongs to the application, not to the pack that declared the
-- facet. Every system can bind an ability to something; a pack saying
-- `binds: item` does not make the *relationship* that pack's, any more than
-- declaring a resource makes the resource table its own.
--
-- It is **not** merged with world_item_effects. An effect is a mechanical rule
-- the resolution layer consumes; an ability is named, described, permissioned,
-- shareable content. They are reconciled where a Game Master meets the
-- confusion — on the item, as one list (FR-020) — and nowhere else.
CREATE TABLE world_item_abilities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    item_id UUID NOT NULL REFERENCES world_items(id) ON DELETE CASCADE,
    ability_id UUID REFERENCES world_abilities(id) ON DELETE SET NULL,
    ability_name_snapshot TEXT NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (item_id, ability_id)
);

CREATE INDEX world_item_abilities_item_id_idx ON world_item_abilities(item_id);
CREATE INDEX world_item_abilities_ability_id_idx ON world_item_abilities(ability_id);
