-- Reverses 2026-09-16-220000-0000_collections_on_the_shelf. Collections have
-- nowhere to live without a hash and pages, so they go first, and with them
-- anything a world wrote over one.
DROP TRIGGER compendium_entries_page_follows_origin_trigger ON compendium_entries;
DROP FUNCTION compendium_entries_page_follows_origin();
DELETE FROM compendiums WHERE origin = 'Authored';

ALTER TABLE compendium_entries DROP CONSTRAINT compendium_entries_page_is_one_based;
ALTER TABLE compendium_entries
    ADD CONSTRAINT compendium_entries_page_is_one_based CHECK (page >= 1);
ALTER TABLE compendium_entries ALTER COLUMN page SET NOT NULL;

ALTER TABLE world_books ALTER COLUMN base_source_hash SET NOT NULL;

ALTER TABLE compendiums DROP CONSTRAINT compendiums_hash_is_for_books;
ALTER TABLE compendiums ALTER COLUMN source_hash SET NOT NULL;
