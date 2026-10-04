-- The trigger as it was, with no detachment allowed.
CREATE OR REPLACE FUNCTION world_entry_deltas_origin_follows_form() RETURNS TRIGGER AS $$
DECLARE
    book_origin "ContentOrigin";
    expected "ContentOrigin";
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF NEW.origin IS DISTINCT FROM OLD.origin THEN
            RAISE EXCEPTION
                'a delta''s origin is not editable (spec 050 FR-052): % cannot become %',
                OLD.origin, NEW.origin;
        END IF;
        IF (NEW.world_id, NEW.compendium_id, NEW.kind, NEW.name)
            IS DISTINCT FROM (OLD.world_id, OLD.compendium_id, OLD.kind, OLD.name) THEN
            RAISE EXCEPTION
                'a delta''s identity is not editable (spec 050 FR-025): a rename is a hide and an addition';
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

DROP TRIGGER IF EXISTS compendiums_detach_additions_trigger ON compendiums;
DROP FUNCTION IF EXISTS world_entry_deltas_detach_additions();

-- An addition whose book was removed has no book to point at under the old
-- shape; it goes, which is the defect this migration fixed and the reason not
-- to roll it back.
DELETE FROM world_entry_deltas WHERE compendium_id IS NULL;

ALTER TABLE world_entry_deltas DROP CONSTRAINT world_entry_deltas_compendium_id_fkey;
ALTER TABLE world_entry_deltas
    ADD CONSTRAINT world_entry_deltas_compendium_id_fkey
        FOREIGN KEY (compendium_id) REFERENCES compendiums(id) ON DELETE CASCADE;
ALTER TABLE world_entry_deltas DROP CONSTRAINT world_entry_deltas_a_book_or_its_name;
ALTER TABLE world_entry_deltas DROP CONSTRAINT world_entry_deltas_only_additions_outlive_a_book;
ALTER TABLE world_entry_deltas ALTER COLUMN compendium_id SET NOT NULL;
ALTER TABLE world_entry_deltas DROP COLUMN written_beside_title;
