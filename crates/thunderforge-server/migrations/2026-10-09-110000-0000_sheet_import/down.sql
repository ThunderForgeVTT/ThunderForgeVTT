ALTER TABLE world_items DROP COLUMN IF EXISTS weight;

ALTER TABLE world_actor_inventory
    DROP CONSTRAINT IF EXISTS one_target,
    DROP COLUMN IF EXISTS attuned,
    DROP COLUMN IF EXISTS equipped,
    DROP COLUMN IF EXISTS staged_id;

ALTER TABLE world_actor_abilities
    DROP CONSTRAINT IF EXISTS one_target,
    DROP COLUMN IF EXISTS recharge,
    DROP COLUMN IF EXISTS uses_used,
    DROP COLUMN IF EXISTS uses_max,
    DROP COLUMN IF EXISTS granted_by,
    DROP COLUMN IF EXISTS prepared,
    DROP COLUMN IF EXISTS staged_id;

DROP TABLE IF EXISTS world_staged_content;
DROP TABLE IF EXISTS actor_imports;
DROP TABLE IF EXISTS sheet_import_versions;
DROP TABLE IF EXISTS brought_characters;
DROP TYPE IF EXISTS "ActorImportKind";
DROP TYPE IF EXISTS "StagedState";
