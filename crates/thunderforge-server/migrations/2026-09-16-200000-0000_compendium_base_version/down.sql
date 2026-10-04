-- Reverses 2026-09-16-200000-0000_compendium_base_version. Which reading of a
-- book is in force stops being numbered; the entries are untouched.
ALTER TABLE compendiums
    DROP CONSTRAINT compendiums_base_version_counts_from_one,
    DROP COLUMN base_version;
