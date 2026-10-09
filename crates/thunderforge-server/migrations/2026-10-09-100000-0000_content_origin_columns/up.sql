-- Spec 048 FR-033a-d: actors, items and abilities answer for their own
-- origin, so a character read from an uploaded sheet cannot leave the world
-- through a collection (spec 049 FR-054a). Everything already written was
-- authored here, which is what the backfill says.

ALTER TABLE world_actors    ADD COLUMN origin "ContentOrigin" NOT NULL DEFAULT 'Authored';
ALTER TABLE world_items     ADD COLUMN origin "ContentOrigin" NOT NULL DEFAULT 'Authored';
ALTER TABLE world_abilities ADD COLUMN origin "ContentOrigin" NOT NULL DEFAULT 'Authored';
-- FR-033d: no permissive default once the backfill is done. A writer states
-- where its content came from, or its insert fails.
ALTER TABLE world_actors    ALTER COLUMN origin DROP DEFAULT;
ALTER TABLE world_items     ALTER COLUMN origin DROP DEFAULT;
ALTER TABLE world_abilities ALTER COLUMN origin DROP DEFAULT;

-- FR-033b: an item's or an ability's origin is fixed when it is written.
CREATE FUNCTION forbid_origin_change() RETURNS trigger AS $$
BEGIN
    IF NEW.origin IS DISTINCT FROM OLD.origin THEN
        RAISE EXCEPTION 'origin is immutable' USING ERRCODE = 'check_violation';
    END IF;
    RETURN NEW;
END $$ LANGUAGE plpgsql;

-- FR-033c: an applied import makes an actor uploaded; nothing makes it
-- authored again, a rollback included.
CREATE FUNCTION actor_origin_only_uploads() RETURNS trigger AS $$
BEGIN
    IF NEW.origin IS DISTINCT FROM OLD.origin
       AND NOT (OLD.origin = 'Authored' AND NEW.origin = 'Uploaded') THEN
        RAISE EXCEPTION 'actor origin may only become Uploaded' USING ERRCODE = 'check_violation';
    END IF;
    RETURN NEW;
END $$ LANGUAGE plpgsql;

CREATE TRIGGER world_items_origin_immutable
BEFORE UPDATE OF origin ON world_items
FOR EACH ROW EXECUTE FUNCTION forbid_origin_change();

CREATE TRIGGER world_abilities_origin_immutable
BEFORE UPDATE OF origin ON world_abilities
FOR EACH ROW EXECUTE FUNCTION forbid_origin_change();

CREATE TRIGGER world_actors_origin_only_uploads
BEFORE UPDATE OF origin ON world_actors
FOR EACH ROW EXECUTE FUNCTION actor_origin_only_uploads();

-- The function as `2026-09-13-140000-0000_additions_outlive_the_book` wrote
-- it, with the actor, item and ability arms reading the column.
CREATE OR REPLACE FUNCTION content_origin(content_type TEXT, content_id UUID)
RETURNS "ContentOrigin" AS $$
BEGIN
    CASE content_type
        WHEN 'compendium' THEN
            RETURN (SELECT origin FROM compendiums WHERE id = content_id);
        WHEN 'compendium_entry' THEN
            RETURN (
                SELECT c.origin
                FROM compendium_entries e
                JOIN compendiums c ON c.id = e.compendium_id
                WHERE e.id = content_id
            );
        -- Spec 050's deltas answer for themselves, per entry (FR-052,
        -- FR-052a): a change or a hide carries its book's origin, an addition
        -- is authored, and the column was set by a trigger that will not let
        -- it be anything else or be flipped afterwards.
        WHEN 'world_entry_delta' THEN
            RETURN (SELECT origin FROM world_entry_deltas WHERE id = content_id);
        -- Spec 048 FR-033a: actors, items and abilities carry their own
        -- origin. A character read from an uploaded sheet, and anything read
        -- with it, is uploaded content.
        WHEN 'actor' THEN
            RETURN (SELECT origin FROM world_actors WHERE id = content_id);
        WHEN 'item' THEN
            RETURN (SELECT origin FROM world_items WHERE id = content_id);
        WHEN 'ability' THEN
            RETURN (SELECT origin FROM world_abilities WHERE id = content_id);
        WHEN 'lore' THEN
            RETURN (SELECT 'Authored'::"ContentOrigin" FROM world_lore_entries WHERE id = content_id);
        WHEN 'scene' THEN
            RETURN (SELECT 'Authored'::"ContentOrigin" FROM scenes WHERE scene_id = content_id);
        ELSE
            RETURN NULL;
    END CASE;
END;
$$ LANGUAGE plpgsql STABLE;
