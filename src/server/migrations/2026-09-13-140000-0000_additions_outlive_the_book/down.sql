-- The invariant, as it was before deltas answered to it.
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
        -- EXTENSION POINT for spec 050's deltas (world_entry_deltas, which
        -- stores its own per-entry origin): a migration after this one
        -- replaces the function with this arm added —
        --   WHEN 'world_entry_delta' THEN
        --       RETURN (SELECT origin FROM world_entry_deltas WHERE id = content_id);
        -- and `origin::tests` gains the matching case.
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

-- A row switched on by somebody since deleted has nobody to restore; it goes,
-- rather than blocking the rollback.
DELETE FROM world_books WHERE switched_on_by IS NULL;
ALTER TABLE world_books DROP CONSTRAINT world_books_switched_on_by_fkey;
ALTER TABLE world_books
    ADD CONSTRAINT world_books_switched_on_by_fkey
        FOREIGN KEY (switched_on_by) REFERENCES users(id);
ALTER TABLE world_books ALTER COLUMN switched_on_by SET NOT NULL;

-- An addition whose book is switched off has no book-list row to hang from
-- under the old shape; it goes, which is exactly the defect this migration
-- fixed, and the reason not to roll it back.
DELETE FROM world_entry_deltas d
 WHERE NOT EXISTS (
     SELECT 1 FROM world_books b
      WHERE b.world_id = d.world_id AND b.compendium_id = d.compendium_id
 );
DROP INDEX IF EXISTS world_entry_deltas_compendium_id_idx;
ALTER TABLE world_entry_deltas DROP CONSTRAINT world_entry_deltas_compendium_id_fkey;
ALTER TABLE world_entry_deltas DROP CONSTRAINT world_entry_deltas_world_id_fkey;
ALTER TABLE world_entry_deltas DROP CONSTRAINT world_entry_deltas_changes_are_on_the_book_list;
ALTER TABLE world_entry_deltas DROP COLUMN book_list_compendium_id;
ALTER TABLE world_entry_deltas
    ADD CONSTRAINT world_entry_deltas_on_the_book_list
        FOREIGN KEY (world_id, compendium_id)
        REFERENCES world_books(world_id, compendium_id)
        ON DELETE CASCADE;
