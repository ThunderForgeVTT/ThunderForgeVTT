-- Restore content_origin() as `2026-09-13-140000-0000_additions_outlive_the_book`
-- wrote it, before the columns it reads are dropped.
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
        WHEN 'actor' THEN
            RETURN (SELECT 'Authored'::"ContentOrigin" FROM world_actors WHERE id = content_id);
        WHEN 'item' THEN
            RETURN (SELECT 'Authored'::"ContentOrigin" FROM world_items WHERE id = content_id);
        WHEN 'ability' THEN
            RETURN (SELECT 'Authored'::"ContentOrigin" FROM world_abilities WHERE id = content_id);
        WHEN 'lore' THEN
            RETURN (SELECT 'Authored'::"ContentOrigin" FROM world_lore_entries WHERE id = content_id);
        WHEN 'scene' THEN
            RETURN (SELECT 'Authored'::"ContentOrigin" FROM scenes WHERE scene_id = content_id);
        ELSE
            RETURN NULL;
    END CASE;
END;
$$ LANGUAGE plpgsql STABLE;

DROP TRIGGER IF EXISTS world_actors_origin_only_uploads ON world_actors;
DROP TRIGGER IF EXISTS world_abilities_origin_immutable ON world_abilities;
DROP TRIGGER IF EXISTS world_items_origin_immutable ON world_items;
DROP FUNCTION IF EXISTS actor_origin_only_uploads();
DROP FUNCTION IF EXISTS forbid_origin_change();

ALTER TABLE world_abilities DROP COLUMN IF EXISTS origin;
ALTER TABLE world_items     DROP COLUMN IF EXISTS origin;
ALTER TABLE world_actors    DROP COLUMN IF EXISTS origin;
