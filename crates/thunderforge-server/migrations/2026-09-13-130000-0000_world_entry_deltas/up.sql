-- Spec 050 (FR-020 to FR-028, FR-052, FR-052a): a world's own changes to the
-- books it inherited.
--
-- One table and one Postgres enum. Three things are worth saying plainly,
-- because each is a decision the arc cannot take back cheaply:
--
-- **A delta is not a copy.** The base stays in `compendium_entries`, on its
-- owner's shelf, immutable while it is the base (FR-006). A row here holds
-- what this world says differently and nothing else (FR-023): a changed
-- creature's armour and not its other forty fields, a hidden entry's identity
-- and no content at all. Phase 9 measured a world's link to a book at 144
-- bytes; a delta table that quietly stored whole entries would have turned
-- that saving back into a copy one edit at a time.
--
-- **A delta attaches to kind and name, never to an entry's id** (FR-025).
-- An id is the row a particular reading produced, and a re-import replaces
-- every row (FR-026). Kind and name are what the book says, and spec 049's
-- Phase 10 measured them stable across a genuine re-parse for every real
-- entry. They are not always *unique* — 3.9% of entries share both with
-- another entry in the same book — and a delta over such an identity must
-- refuse to attach rather than guess (FR-025a). That refusal is the
-- application's, at write time and at read time, because the base can gain a
-- twin underneath an existing delta by being re-read, and no constraint on
-- this table can see that happen.
--
-- **Origin is per entry, not per book** (FR-052, FR-052a). A changed or hidden
-- entry is a mutation of something in the book and carries the book's origin:
-- editing an uploaded sword does not make it shareable. An added entry was
-- written by a person in this world and is authored, whatever book it sits
-- beside. The two rows look almost identical, sit on the same screen and are
-- made by the same people, and they carry opposite rights — so the origin is
-- a column, and a trigger below decides it rather than trusting a caller to.

-- The three forms a delta takes (FR-021).
--
-- An enum rather than a VARCHAR for the reason `ContentOrigin` is one: the
-- set is closed, and each value decides which columns below may hold
-- anything. A fourth spelling would be a row no check constraint describes.
-- PascalCase follows "ContentOrigin".
CREATE TYPE "DeltaForm" AS ENUM ('Changed', 'Hidden', 'Added');

CREATE TABLE world_entry_deltas (
    id UUID PRIMARY KEY,

    -- The world and the book, as one reference to the book list rather than
    -- two references to the world and the compendium.
    --
    -- A delta over a book this world has not switched on is not a lesser
    -- delta, it is meaningless, and a composite foreign key makes it
    -- unrepresentable. The cascade then says three things with one clause:
    --
    -- * a world that goes takes its deltas and nothing of any base or any
    --   other world (FR-028) — through `world_books`, which cascades from
    --   `worlds`;
    -- * a compendium removed from the shelf takes every world's deltas over
    --   it (FR-061) — through the same row, from the other side;
    -- * switching a book off takes this world's changes to it (FR-013). That
    --   last one is a real loss, including of additions a person wrote, which
    --   is why the switch-off report names every delta *before* the link goes.
    --   A cascade that reported what it broke afterwards would not be a
    --   confirmation.
    world_id UUID NOT NULL,
    compendium_id UUID NOT NULL,

    -- The identity this delta attaches to (FR-025). For an addition, the
    -- identity it introduces.
    kind VARCHAR(64) NOT NULL,
    name TEXT NOT NULL,

    form "DeltaForm" NOT NULL,

    -- Where this entry, as the world reads it, came from — decided by the
    -- trigger below from `form` and the book, never chosen by a caller.
    --
    -- Stored rather than derived at every read because it is the value a
    -- sharing check asks for by delta id, from a module that should not have
    -- to re-learn the delta model to answer. It cannot drift from the thing
    -- it is derived from: a compendium's origin is immutable (spec 049
    -- FR-057), and so is this one.
    origin "ContentOrigin" NOT NULL,

    -- What differs, and only that (FR-023).
    --
    -- *Changed*, anchored entry: the declared fields this world set to a
    --   different value, in the same tagged shape `compendium_entries`
    --   stores, so that a field the world marks "not found" is still absence
    --   and not an empty string. The fields it did not touch are not here.
    -- *Changed*, prose entry: `prose_text`, and no fields at all.
    -- *Hidden*: nothing. A hidden entry needs its identity and no content.
    -- *Added*: the whole entry, because the whole entry is the difference —
    --   there is no base to be different from.
    --
    -- Named `field_values` to match the base: VALUES is reserved in SQL.
    field_values JSONB,
    prose_text TEXT,

    -- Who made the change. SET NULL rather than a hard reference because a
    -- delta is the world's, not its author's: a Trusted Player deleting their
    -- account must neither take a table's changes with them nor be refused
    -- the deletion because they once retuned a goblin.
    changed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT world_entry_deltas_on_the_book_list
        FOREIGN KEY (world_id, compendium_id)
        REFERENCES world_books(world_id, compendium_id)
        ON DELETE CASCADE,

    -- One delta per identity per world per book. Two rows over one identity
    -- would be a world that says two different things about one goblin, and
    -- which one it reads would come down to the order of a query. It also
    -- keeps additions identifiable: two additions sharing a kind and name
    -- would be exactly the ambiguity FR-025a refuses in a base.
    CONSTRAINT world_entry_deltas_one_per_identity
        UNIQUE (world_id, compendium_id, kind, name),

    -- An identity nobody can see is an identity nobody can restore.
    CONSTRAINT world_entry_deltas_name_is_not_blank CHECK (btrim(name) <> ''),

    CONSTRAINT world_entry_deltas_fields_are_an_object CHECK (
        field_values IS NULL OR jsonb_typeof(field_values) = 'object'
    ),

    -- A hidden entry carries no content. Were it allowed to, "hidden" would
    -- quietly become "changed and not shown", and restoring it would bring
    -- back something other than the book.
    CONSTRAINT world_entry_deltas_hidden_is_empty CHECK (
        form <> 'Hidden' OR (field_values IS NULL AND prose_text IS NULL)
    ),

    -- A change changes exactly one of the two things an entry can hold, and
    -- changes it to something. An empty change is not a change — the
    -- application deletes the row instead, so the base shows through and
    -- "changed" never marks an entry identical to its book. And fields on a
    -- prose entry, or prose on an anchored one, are FR-001b's rule from the
    -- base carried into the delta: the importer may not invent fields for
    -- prose, and neither may an edit.
    CONSTRAINT world_entry_deltas_changed_changes_one_thing CHECK (
        form <> 'Changed' OR (
            (field_values IS NULL) <> (prose_text IS NULL)
            AND (field_values IS NULL OR field_values <> '{}'::jsonb)
        )
    ),

    -- An addition is a whole entry, and the same implication the base states:
    -- prose has no mechanical fields.
    CONSTRAINT world_entry_deltas_added_is_whole CHECK (
        form <> 'Added' OR (
            field_values IS NOT NULL
            AND (prose_text IS NULL OR field_values = '{}'::jsonb)
        )
    )
);

-- The one read the resolution makes — every delta a world holds over one
-- book — is served by the unique constraint's index, which `world_id` and
-- `compendium_id` lead. A second index on the same prefix would be a second
-- thing to write on every edit and nothing gained.

-- FR-052 and FR-052a, made unrepresentable rather than merely checked.
--
-- The origin a row may carry is a function of its form and its book:
--
-- * **Changed** and **Hidden** carry the book's origin. A mutation has no
--   meaning apart from the thing it mutates, and an uploaded entry must not
--   become shareable by being edited.
-- * **Added** is **Authored**. A person wrote it here.
--
-- The application writes the right value, and phrases every refusal for a
-- person; this is what stops the value being got wrong by a route nobody
-- thought of. On update the origin may not change at all, which is also what
-- stops a change being turned into an addition in place: "uploaded becomes
-- authored" is precisely the flip that turns a publisher's creature into
-- something offered to strangers, spec 049 FR-057's reason for the matching
-- trigger on `compendiums`. The identity is fixed on update too — a rename is
-- a hide and an addition, and must look like one.
--
-- Hard errors, not silent corrections, for the reason the compendium trigger
-- gives: a write that was refused must look refused from the caller's side.
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

CREATE TRIGGER world_entry_deltas_origin_trigger
BEFORE INSERT OR UPDATE ON world_entry_deltas
FOR EACH ROW EXECUTE PROCEDURE world_entry_deltas_origin_follows_form();
