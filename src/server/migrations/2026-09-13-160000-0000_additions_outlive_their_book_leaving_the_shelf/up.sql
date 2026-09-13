-- Spec 050 decision 5, carried from switching a book off to removing it from
-- the shelf (owner, 2026-09-13; FR-061 amended to match).
--
-- `2026-09-13-140000-0000_additions_outlive_the_book` kept a table's additions
-- when a book was switched off, and still let them cascade away when the book
-- itself was deleted. The reasoning that kept them the first time applies with
-- more force the second: an addition is a table's own homebrew, written beside
-- a book rather than derived from it, and removing a PDF from somebody's shelf
-- is no reason for a table to lose what it wrote. Changes and hides still go
-- — they mean nothing without the entries they modify.
--
-- **What an addition holds once its book is gone.** Its `compendium_id`
-- becomes NULL and the book's title at that moment is copied into
-- `written_beside_title`. The two are exclusive: an addition either points at
-- a book that exists or remembers the name of one that does not, never both
-- and never neither. A snapshot rather than a tombstone row in `compendiums`,
-- because a tombstone would be a book that exists for some purposes and not
-- others — a row a shelf query can forget to filter, and a way back to content
-- that was deleted. A title is a name, not a book, and nothing can be read out
-- of it.
--
-- **No way back.** A detached addition cannot be attached to anything again,
-- including a later re-import of the same file, which is a new compendium with
-- a new id. The trigger below refuses the transition in that direction. The
-- addition stays under "Written at this table" for good.

ALTER TABLE world_entry_deltas
    ADD COLUMN written_beside_title VARCHAR(300);

ALTER TABLE world_entry_deltas
    ALTER COLUMN compendium_id DROP NOT NULL;

-- A change or a hide always names a book: it is a change *to* something.
ALTER TABLE world_entry_deltas
    ADD CONSTRAINT world_entry_deltas_only_additions_outlive_a_book
        CHECK (form = 'Added' OR compendium_id IS NOT NULL);

-- Either the book, or the name of the book that was. Exactly one.
ALTER TABLE world_entry_deltas
    ADD CONSTRAINT world_entry_deltas_a_book_or_its_name
        CHECK ((compendium_id IS NULL) <> (written_beside_title IS NULL));

-- The reference to `compendiums` no longer cascades, and it is checked at
-- commit rather than at the end of the statement.
--
-- Not SET NULL: that would null a change's book as readily as an addition's,
-- and the check above would refuse the whole removal. Not an immediate check
-- either, measured rather than assumed: the changes and hides over a removed
-- book go by a cascade two steps away (compendium, then the book-list row,
-- then the delta), and an immediate check on this key runs before that second
-- step has, so every removal of a book with a change over it was refused.
-- Deferred, the order of the cascades stops mattering. The trigger below
-- detaches additions, the book-list cascade takes changes and hides, and
-- anything still pointing at the book when the transaction commits is an
-- error rather than a dangling id.
ALTER TABLE world_entry_deltas
    DROP CONSTRAINT world_entry_deltas_compendium_id_fkey;

ALTER TABLE world_entry_deltas
    ADD CONSTRAINT world_entry_deltas_compendium_id_fkey
        FOREIGN KEY (compendium_id) REFERENCES compendiums(id)
        DEFERRABLE INITIALLY DEFERRED;

-- Detach a book's additions as it leaves the shelf — by any route: the
-- library's removal, an account deletion's cascade, or a DELETE in psql.
--
-- A trigger because the thing being decided is what a removal *does* to rows
-- that outlive it, and a foreign key can only delete, null or refuse. If this
-- trigger were dropped, removal would fail on the foreign key above at commit
-- rather than silently take anybody's writing, which is the direction a
-- missing trigger should fail in.
CREATE OR REPLACE FUNCTION world_entry_deltas_detach_additions() RETURNS TRIGGER AS $$
BEGIN
    UPDATE world_entry_deltas
       SET compendium_id = NULL,
           written_beside_title = OLD.book_title,
           updated_at = CURRENT_TIMESTAMP
     WHERE compendium_id = OLD.id
       AND form = 'Added';
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER compendiums_detach_additions_trigger
BEFORE DELETE ON compendiums
FOR EACH ROW EXECUTE PROCEDURE world_entry_deltas_detach_additions();

-- The delta trigger from `2026-09-13-130000-0000_world_entry_deltas`, with one
-- transition added to what identity may do on update: an addition may lose its
-- book, once, and gain the name of it. Every other change of identity is still
-- refused, and in particular a detached addition may never gain a book again.
CREATE OR REPLACE FUNCTION world_entry_deltas_origin_follows_form() RETURNS TRIGGER AS $$
DECLARE
    book_origin "ContentOrigin";
    expected "ContentOrigin";
    detaching BOOLEAN;
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF NEW.origin IS DISTINCT FROM OLD.origin THEN
            RAISE EXCEPTION
                'a delta''s origin is not editable (spec 050 FR-052): % cannot become %',
                OLD.origin, NEW.origin;
        END IF;

        detaching := OLD.form = 'Added'
            AND NEW.form = 'Added'
            AND OLD.compendium_id IS NOT NULL
            AND NEW.compendium_id IS NULL
            AND OLD.written_beside_title IS NULL
            AND NEW.written_beside_title IS NOT NULL;

        IF NOT detaching AND (
            (NEW.world_id, NEW.compendium_id, NEW.kind, NEW.name, NEW.written_beside_title)
            IS DISTINCT FROM
            (OLD.world_id, OLD.compendium_id, OLD.kind, OLD.name, OLD.written_beside_title)
        ) THEN
            RAISE EXCEPTION
                'a delta''s identity is not editable (spec 050 FR-025): a rename is a hide and an addition, and an addition whose book was removed cannot rejoin one';
        END IF;
    END IF;

    SELECT origin INTO book_origin FROM compendiums WHERE id = NEW.compendium_id;

    expected := CASE NEW.form
        WHEN 'Added' THEN 'Authored'::"ContentOrigin"
        ELSE book_origin
    END;

    IF NEW.origin IS DISTINCT FROM expected THEN
        RAISE EXCEPTION
            'a % entry over a % book is %, not % (spec 050 FR-052, FR-052a)',
            NEW.form, book_origin, expected, NEW.origin;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
