-- Spec 050 (FR-010 to FR-015, FR-030 to FR-036, FR-041, FR-042): the book
-- list. Which compendiums a world has switched on, the way a mod list says
-- what a server is running.
--
-- One table, and the whole of what a world gets from a library is in it.
-- Three things are worth saying plainly, because each is a decision the arc
-- cannot take back cheaply:
--
-- **Nothing is copied.** There is no content column here and there will never
-- be one. A row says "this world reads that book"; the entries stay in
-- `compendium_entries`, owned by the account, and a second world switching the
-- same book on adds one row of about a hundred bytes rather than a second copy
-- of a Monster Manual (FR-011, FR-031, FR-070). The absence of copied content
-- is the point of the feature, so the absence of a column to put it in is the
-- feature's load-bearing part.
--
-- **The row is the world's, the book is the account's.** This table is deleted
-- by the world going (FR-028) and by the compendium going (FR-061), and in
-- neither direction does the deletion reach through to the other's content.
--
-- **A world draws only on its own owner's shelf** (FR-014). That is enforced
-- below by a trigger rather than by the application alone, because it is the
-- crossing where account scope meets world scope and the one place in this arc
-- where getting it wrong means one person's book reaching another person's
-- account.

CREATE TABLE world_books (
    id UUID PRIMARY KEY,

    -- The table this book is on for. CASCADE is FR-028: a world that goes
    -- takes its book list with it, and takes nothing of the library.
    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,

    -- The book, on its owner's shelf. CASCADE is FR-061: removing a
    -- compendium takes every world's link to it, because a link to a book
    -- that no longer exists is not a lesser link, it is a dangling one.
    --
    -- Naming what is in use *before* that removal happens (FR-013, FR-060) is
    -- the application's, and it cannot be folded in here: a cascade that
    -- reports what it would break and then breaks it anyway is not a
    -- confirmation.
    compendium_id UUID NOT NULL REFERENCES compendiums(id) ON DELETE CASCADE,

    -- Which base version was in force when this was switched on (FR-015).
    --
    -- Recorded as the pair that actually identifies a base today: the file's
    -- SHA-256 and the build of the reader that read it. There is deliberately
    -- no version counter on `compendiums` to point at, because a base is
    -- replaced wholesale by a re-import (FR-006) and the two columns that
    -- change when it is are exactly these two. A re-import of the same file
    -- under an improved reader moves `base_parser_version`; a different file
    -- is a different book and is refused as an overwrite already (049 FR-047).
    --
    -- Stored rather than joined so that "which base was this world reading?"
    -- survives the base being replaced underneath it — the question spec 050
    -- US5 exists to answer, and one a join to the current row cannot.
    base_source_hash VARCHAR(64) NOT NULL,
    base_parser_version VARCHAR(64) NOT NULL,

    -- Who switched it on and when (FR-015). The Game Master, always: this
    -- table's trigger will not accept a row whose compendium is not the
    -- world owner's, so there is no path by which a co-Game Master's shelf
    -- reaches somebody else's world.
    switched_on_by UUID NOT NULL REFERENCES users(id),
    switched_on_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

    -- Switched on twice is switched on once. FR-033 asks that ticking a book
    -- at world creation and ticking it afterwards produce the same state;
    -- this is what makes "the same state" a thing the database can be asked
    -- about rather than a property two code paths have to keep agreeing on.
    CONSTRAINT world_books_unique_per_world UNIQUE (world_id, compendium_id)
);

-- The two reads this table has: what is on for one world (the book list every
-- member sees, FR-030, FR-035) and which worlds a compendium is on for (the
-- removal report, FR-060).
CREATE INDEX world_books_world_id_idx ON world_books(world_id);
CREATE INDEX world_books_compendium_id_idx ON world_books(compendium_id);

-- FR-014 and FR-041, made unrepresentable rather than merely checked.
--
-- A world may switch on a book if, and only if, **the world's owner holds
-- it** and **the book was read as the world's system**. The application asks
-- both questions too, and phrases the refusal for a person; this is what
-- stops the answer being got wrong by a route nobody thought of — a repair
-- script, a psql session, a future resolver assembled from a struct that
-- grew a field.
--
-- Two properties of the check are deliberate:
--
-- *Ownership is the world's owner, not the caller.* A co-Game Master may use
-- what is switched on at a table they run and may not put their own books on
-- it, nor take the owner's home: both follow from comparing
-- `worlds.created_by` with `compendiums.owner_user_id` and never consulting
-- who is making the request. Being able to use a book is not being able to
-- take it home.
--
-- *It fires on insert and update only.* A world that changes system afterwards
-- keeps its rows, which is FR-042 working rather than failing: what no longer
-- matches must be **reported** to the Game Master, not deleted out from under
-- them and not quietly served either. The serving half is the application's;
-- what this guarantees is that a mismatch can only ever arrive that way, never
-- by being switched on.
CREATE OR REPLACE FUNCTION world_books_from_the_owners_shelf() RETURNS TRIGGER AS $$
DECLARE
    world_owner UUID;
    world_system VARCHAR(64);
    book_owner UUID;
    book_system VARCHAR(64);
BEGIN
    SELECT created_by, game_system_id INTO world_owner, world_system
    FROM worlds WHERE id = NEW.world_id;

    SELECT owner_user_id, system_id INTO book_owner, book_system
    FROM compendiums WHERE id = NEW.compendium_id;

    IF world_owner IS DISTINCT FROM book_owner THEN
        RAISE EXCEPTION
            'a world may only switch on a compendium its owner holds (spec 050 FR-014)';
    END IF;

    -- A world with no system chosen matches nothing, rather than matching
    -- everything. A book read as Pathfinder says nothing intelligible to a
    -- world that has not said what it is.
    IF world_system IS NULL OR world_system IS DISTINCT FROM book_system THEN
        RAISE EXCEPTION
            'a compendium read as % cannot be switched on in a world running % (spec 050 FR-041)',
            book_system, COALESCE(world_system, 'no system');
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER world_books_owner_and_system_trigger
BEFORE INSERT OR UPDATE ON world_books
FOR EACH ROW EXECUTE PROCEDURE world_books_from_the_owners_shelf();
