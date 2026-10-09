-- Spec 048: bringing a character in from a sheet.
--
-- A brought character belongs to its player's account (FR-030): the file and
-- every reading of it are theirs, kept as numbered versions. An import into a
-- world is recorded with the actor as it was before, so it can be rolled back
-- (FR-040). Content read from a sheet that the world does not already have is
-- staged for the GM (FR-033), uploaded by origin, and adopted or declined.

CREATE TYPE "StagedState" AS ENUM ('pending', 'adopted', 'declined');
CREATE TYPE "ActorImportKind" AS ENUM ('import', 'rollback');

CREATE TABLE brought_characters (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    system_id     varchar(64) NOT NULL,
    name          varchar(200) NOT NULL,
    created_at    timestamp NOT NULL DEFAULT now(),
    updated_at    timestamp NOT NULL DEFAULT now(),
    created_by    uuid NOT NULL REFERENCES users(id),
    updated_by    uuid NOT NULL REFERENCES users(id)
);
CREATE INDEX brought_characters_owner ON brought_characters (owner_user_id);

CREATE TABLE sheet_import_versions (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    character_id   uuid NOT NULL REFERENCES brought_characters(id) ON DELETE CASCADE,
    version_no     integer NOT NULL CHECK (version_no >= 1),
    -- sheets/{owner}/{character}/{version}.pdf
    file_key       text NOT NULL UNIQUE,
    file_sha256    char(64) NOT NULL,
    file_bytes     integer NOT NULL CHECK (file_bytes > 0),
    file_pages     smallint NOT NULL CHECK (file_pages > 0),
    reader_id      varchar(64) NOT NULL,
    reader_version varchar(32) NOT NULL,
    -- The server's own reading, never the browser's.
    reading        jsonb NOT NULL,
    -- The player's corrections, kept apart from what was read.
    corrections    jsonb NOT NULL DEFAULT '{}',
    created_at     timestamp NOT NULL DEFAULT now(),
    created_by     uuid NOT NULL REFERENCES users(id),
    updated_by     uuid NOT NULL REFERENCES users(id),
    UNIQUE (character_id, version_no)
);

CREATE TABLE actor_imports (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    world_id        uuid NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    actor_id        uuid NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    version_id      uuid REFERENCES sheet_import_versions(id) ON DELETE SET NULL,
    kind            "ActorImportKind" NOT NULL,
    restored_from   uuid REFERENCES actor_imports(id),
    -- Sheet fields and links as they were before this was applied.
    before_snapshot jsonb NOT NULL,
    -- The plan's changes as applied.
    written         jsonb NOT NULL,
    plan_hash       char(64),
    applied_at      timestamp NOT NULL DEFAULT now(),
    created_by      uuid NOT NULL REFERENCES users(id),
    updated_by      uuid NOT NULL REFERENCES users(id),
    CHECK ((kind = 'import'   AND version_id IS NOT NULL AND restored_from IS NULL)
        OR (kind = 'rollback' AND restored_from IS NOT NULL))
);
CREATE INDEX actor_imports_actor ON actor_imports (actor_id, applied_at DESC);

CREATE TABLE world_staged_content (
    id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    world_id           uuid NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    player_user_id     uuid NOT NULL REFERENCES users(id),
    -- A vocabulary type, or 'item'.
    kind               varchar(32) NOT NULL,
    name               varchar(200) NOT NULL,
    normalized_name    varchar(200) NOT NULL,
    content_hash       char(64) NOT NULL,
    field_values       jsonb NOT NULL,
    origin             "ContentOrigin" NOT NULL CHECK (origin = 'Uploaded'),
    state              "StagedState" NOT NULL DEFAULT 'pending',
    differs_from       uuid REFERENCES world_staged_content(id),
    first_actor_id     uuid REFERENCES world_actors(id) ON DELETE SET NULL,
    adopted_ability_id uuid REFERENCES world_abilities(id),
    adopted_item_id    uuid REFERENCES world_items(id),
    decided_by         uuid REFERENCES users(id),
    decided_at         timestamp,
    created_at         timestamp NOT NULL DEFAULT now(),
    created_by         uuid NOT NULL REFERENCES users(id),
    updated_by         uuid NOT NULL REFERENCES users(id),
    UNIQUE (world_id, kind, normalized_name, content_hash),
    CHECK ((state = 'adopted') = (adopted_ability_id IS NOT NULL OR adopted_item_id IS NOT NULL))
);
CREATE INDEX world_staged_content_queue ON world_staged_content (world_id, state, player_user_id);

ALTER TABLE world_actor_abilities
    ADD COLUMN staged_id  uuid REFERENCES world_staged_content(id),
    ADD COLUMN prepared   boolean,
    -- "Fighter 2", "Warforged", "Lucky".
    ADD COLUMN granted_by varchar(200),
    ADD COLUMN uses_max   smallint CHECK (uses_max >= 0),
    ADD COLUMN uses_used  smallint CHECK (uses_used >= 0),
    -- short_rest | long_rest | dawn | none
    ADD COLUMN recharge   varchar(16),
    ADD CONSTRAINT one_target CHECK (NOT (ability_id IS NOT NULL AND staged_id IS NOT NULL));

ALTER TABLE world_actor_inventory
    ADD COLUMN staged_id uuid REFERENCES world_staged_content(id),
    ADD COLUMN equipped  boolean NOT NULL DEFAULT false,
    ADD COLUMN attuned   boolean NOT NULL DEFAULT false,
    ADD CONSTRAINT one_target CHECK (NOT (item_id IS NOT NULL AND staged_id IS NOT NULL));

-- Pounds. double precision, as reach and range are, rather than numeric:
-- the server's Diesel has no numeric support and a weight needs none.
ALTER TABLE world_items ADD COLUMN weight double precision CHECK (weight >= 0);
