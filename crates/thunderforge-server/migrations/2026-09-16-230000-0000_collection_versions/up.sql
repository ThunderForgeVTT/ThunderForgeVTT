-- Spec 049 Phase 15, spec 050 FR-104, ADR-098: a collection keeps its earlier
-- versions, so a sync back (or any other change) can be regretted.
--
-- `compendiums.base_version` already names which version is in force, and
-- every change to a collection moves it on. What was missing is the content of
-- the version it moved on *from*. Each row here is one version as it stood the
-- moment before it was replaced, written in the same transaction as the
-- change, so a change that fails leaves no version behind and a change that
-- lands always leaves one.
--
-- **Collections only.** An imported book is re-read, never synced to, and its
-- previous reading is not kept (FR-006 says a new reading replaces it). The
-- trigger below refuses a version of an uploaded book, and every sync back
-- writes a version before it writes anything else, so a sync back aimed at a
-- book is refused by the database whatever route it came by (FR-101).
--
-- **The owner's alone** (ADR-098 condition 1). Nothing here is reachable
-- from a share link or an adoption: a shelf collection has neither. The rows
-- go with the collection, and the collection goes with the account (FR-064).
CREATE TABLE shelf_collection_versions (
    id UUID PRIMARY KEY,
    compendium_id UUID NOT NULL REFERENCES compendiums (id) ON DELETE CASCADE,
    version INTEGER NOT NULL CHECK (version >= 1),
    -- The title the collection had then, so a restored version is recognisable.
    book_title VARCHAR(300) NOT NULL,
    -- Every entry of that version: kind, name and what it said.
    entries JSONB NOT NULL,
    entry_counts JSONB NOT NULL,
    -- What replaced this version, in words its owner reads.
    replaced_by VARCHAR(300) NOT NULL,
    replaced_at TIMESTAMP NOT NULL DEFAULT now(),
    UNIQUE (compendium_id, version)
);

CREATE OR REPLACE FUNCTION shelf_collection_versions_only_authored() RETURNS TRIGGER AS $$
DECLARE
    book_origin "ContentOrigin";
BEGIN
    SELECT origin INTO book_origin FROM compendiums WHERE id = NEW.compendium_id;
    IF book_origin IS DISTINCT FROM 'Authored' THEN
        RAISE EXCEPTION USING
            ERRCODE = 'check_violation',
            MESSAGE = 'only a collection keeps versions; a book read in records what the book says and is never written back to (spec 050 FR-101)';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER shelf_collection_versions_only_authored_trigger
BEFORE INSERT OR UPDATE OF compendium_id ON shelf_collection_versions
FOR EACH ROW EXECUTE PROCEDURE shelf_collection_versions_only_authored();
