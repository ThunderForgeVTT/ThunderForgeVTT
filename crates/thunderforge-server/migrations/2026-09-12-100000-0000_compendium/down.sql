DROP TABLE IF EXISTS compendium_entries;
DROP TRIGGER IF EXISTS compendiums_origin_immutable_trigger ON compendiums;
DROP FUNCTION IF EXISTS compendiums_origin_is_immutable();
DROP TABLE IF EXISTS compendiums;
DROP TYPE IF EXISTS "ContentOrigin";
