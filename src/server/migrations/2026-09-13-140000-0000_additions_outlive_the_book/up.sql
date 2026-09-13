-- Spec 050 follow-up to Phase 11: three corrections, one migration.
--
-- 1. **Additions outlive the book they were written beside** (decision 5).
--    Phase 11 hung every delta off the book-list row, so switching a book off
--    took a Game Master's own writing with it. Decision 5 had already said
--    otherwise, for a reason that follows from the model: a change or a hide
--    means nothing without the entry it modifies, and an addition never
--    needed the book at all.
-- 2. **Switching a book on does not tie a person to the world for ever.**
--    `world_books.switched_on_by` refused the deletion of any account that had
--    once switched a book on in somebody else's world.
-- 3. **Deltas join the origin invariant** (spec 049 FR-054a, ADR-097), through
--    the extension point the invariant left for them.

-- ---------------------------------------------------------------------------
-- 1. Additions outlive the book
-- ---------------------------------------------------------------------------

-- The reference to the book list, kept for exactly the rows that need it.
--
-- A foreign key cannot say "for some rows". A generated column can: it is the
-- book's id for a change or a hide and NULL for an addition, and a composite
-- foreign key with a NULL in it is not checked (MATCH SIMPLE). So the same
-- clause as before still makes a change or a hide over a book this world is
-- not running unrepresentable, and still takes them when the book is switched
-- off — and an addition is simply not on the end of that rope.
--
-- Chosen over a trigger on `world_books` that deletes the right rows on the
-- way out, because a trigger is an instruction the database follows and a
-- foreign key is a shape it cannot leave. The property that matters — no
-- change or hide survives its book being switched off — is the second kind.
ALTER TABLE world_entry_deltas
    DROP CONSTRAINT world_entry_deltas_on_the_book_list;

ALTER TABLE world_entry_deltas
    ADD COLUMN book_list_compendium_id UUID
        GENERATED ALWAYS AS (CASE WHEN form = 'Added' THEN NULL ELSE compendium_id END) STORED;

ALTER TABLE world_entry_deltas
    ADD CONSTRAINT world_entry_deltas_changes_are_on_the_book_list
        FOREIGN KEY (world_id, book_list_compendium_id)
        REFERENCES world_books(world_id, compendium_id)
        ON DELETE CASCADE;

-- What the book-list reference used to say on every row's behalf, now said
-- directly: a world that goes takes all its deltas, additions included
-- (FR-028), because an addition is the world's and has nowhere else to live.
ALTER TABLE world_entry_deltas
    ADD CONSTRAINT world_entry_deltas_world_id_fkey
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE;

-- And a book removed from the shelf takes every delta over it (FR-061),
-- additions included. This is FR-061 as written; see the note in the Phase 11
-- follow-up report — decision 5's reasoning would keep an addition here too,
-- and the spec has not yet said so.
--
-- `compendium_id` on an addition still names the book it was written beside,
-- which is what lets it rejoin that book's page when the book is switched
-- back on. The unique constraint on (world, book, kind, name) is unchanged,
-- so switching back on cannot produce a second copy: there is one row, and it
-- was never gone.
ALTER TABLE world_entry_deltas
    ADD CONSTRAINT world_entry_deltas_compendium_id_fkey
        FOREIGN KEY (compendium_id) REFERENCES compendiums(id) ON DELETE CASCADE;

CREATE INDEX world_entry_deltas_compendium_id_idx ON world_entry_deltas(compendium_id);

-- ---------------------------------------------------------------------------
-- 2. Who switched a book on may leave
-- ---------------------------------------------------------------------------

-- A book-list row is the world's. Who switched it on is a record (FR-015),
-- and a record of a person who has since deleted their account is a record
-- that says "somebody who is no longer here" — which is true — rather than a
-- reason to refuse that person the deletion. The book stays switched on.
ALTER TABLE world_books
    ALTER COLUMN switched_on_by DROP NOT NULL;

ALTER TABLE world_books
    DROP CONSTRAINT world_books_switched_on_by_fkey;

ALTER TABLE world_books
    ADD CONSTRAINT world_books_switched_on_by_fkey
        FOREIGN KEY (switched_on_by) REFERENCES users(id) ON DELETE SET NULL;

-- ---------------------------------------------------------------------------
-- 3. Deltas under the origin invariant
-- ---------------------------------------------------------------------------

-- The function as `2026-09-13-120000-0000_origin_invariant` wrote it, with the
-- arm its extension point named and nothing else changed.
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
