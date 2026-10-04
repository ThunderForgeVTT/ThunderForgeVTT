-- Spec 031 (T011, US8/FR-036): an actor's imagery, as rows keyed by role.
--
-- Not two columns (`portrait_asset_id`, `token_asset_id`) on world_actors.
-- ADR-057 argues the case: the deferred talking/not-talking/background set is
-- *n* images per actor, and columns would force that into a second mechanism
-- alongside the first. Rows keyed by role make it additive.
--
-- `role` is open text, not an enum or a CHECK, on ADR-054's reasoning — a
-- fixed set means every new role edits the core. The roles this feature needs
-- are 'portrait' and 'token'; a role no code recognises is ignored rather than
-- rendered.
--
-- `asset_id` carries no foreign key, matching world_items.icon_asset_id: the
-- bytes live in the same RustFS-backed object storage every other asset table
-- uses, and no single table owns actor imagery renditions today.
--
-- Provenance follows world_abilities (spec 025): both created_by and
-- updated_by, NOT NULL against users(id), per Constitution Principle III.
CREATE TABLE world_actor_images (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id UUID NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    role VARCHAR NOT NULL,
    asset_id UUID NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- At most one image per (actor, role).
    UNIQUE (actor_id, role)
);

-- The read is always "every image this actor has", so the actor leads.
CREATE INDEX world_actor_images_actor_id_idx ON world_actor_images(actor_id);
