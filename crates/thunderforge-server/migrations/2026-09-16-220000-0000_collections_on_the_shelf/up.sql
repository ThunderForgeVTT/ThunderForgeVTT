-- Spec 049 Phase 13, spec 050 FR-007 to FR-009c: collections on the same
-- shelf as imported books.
--
-- A collection is a `compendiums` row whose origin is 'Authored'. Same table,
-- because FR-008 says a collection behaves identically to a compendium
-- everywhere origin does not decide the answer — the shelf, the book list,
-- a world's read and a world's deltas all take a compendium id, and one table
-- makes "identically" true by construction rather than by keeping two paths
-- in step. What origin does decide is enforced below and in the code.
--
-- Two things a book has and a collection cannot: the hash of the file it was
-- read from, and the page each entry was found on. Both become nullable, and
-- both are tied to origin so neither can be absent from a book or invented
-- for a collection.

ALTER TABLE compendiums ALTER COLUMN source_hash DROP NOT NULL;
ALTER TABLE compendiums
    ADD CONSTRAINT compendiums_hash_is_for_books CHECK (
        (origin = 'Uploaded') = (source_hash IS NOT NULL)
    );

-- The book list recorded the hash of the reading it switched on. A collection
-- has none, so the link to one records none.
ALTER TABLE world_books ALTER COLUMN base_source_hash DROP NOT NULL;

ALTER TABLE compendium_entries ALTER COLUMN page DROP NOT NULL;
ALTER TABLE compendium_entries DROP CONSTRAINT compendium_entries_page_is_one_based;
ALTER TABLE compendium_entries
    ADD CONSTRAINT compendium_entries_page_is_one_based CHECK (page IS NULL OR page >= 1);

-- A page is a promise that a person can look the entry up in the book (049
-- FR-043). An entry of an uploaded book always has one; an entry written in
-- ThunderForge never does. A CHECK cannot see the parent row, so a trigger.
CREATE OR REPLACE FUNCTION compendium_entries_page_follows_origin() RETURNS TRIGGER AS $$
DECLARE
    book_origin "ContentOrigin";
BEGIN
    SELECT origin INTO book_origin FROM compendiums WHERE id = NEW.compendium_id;
    IF book_origin = 'Uploaded' AND NEW.page IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = 'check_violation',
            MESSAGE = 'an entry of an uploaded book must name the page it was read from (spec 049 FR-043)';
    END IF;
    IF book_origin = 'Authored' AND NEW.page IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = 'check_violation',
            MESSAGE = 'an entry written in a collection is on no page of any book (spec 050 FR-007)';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER compendium_entries_page_follows_origin_trigger
BEFORE INSERT OR UPDATE OF page, compendium_id ON compendium_entries
FOR EACH ROW EXECUTE PROCEDURE compendium_entries_page_follows_origin();
