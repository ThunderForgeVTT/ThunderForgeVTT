-- Spec 049 T079, spec 050 FR-006 and FR-026: a re-import is a new version of a
-- book's base, never an edit of the one in force.
--
-- The entries were already replaced whole, in one transaction, so every
-- re-read already produced new rows. What was missing is a name for which
-- reading is in force: Phase 9 found the book list could record only the
-- file's hash and the reader's version, and the hash is the same for every
-- reading of one file by construction. A counter is that name. It starts at 1
-- for every book already on a shelf, and only a re-import moves it.
ALTER TABLE compendiums
    ADD COLUMN base_version INTEGER NOT NULL DEFAULT 1,
    ADD CONSTRAINT compendiums_base_version_counts_from_one CHECK (base_version >= 1);
