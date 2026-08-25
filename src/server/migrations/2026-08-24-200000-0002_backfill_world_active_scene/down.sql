-- Reverse: not attempted — this was a one-time data backfill, not a
-- schema change, and there's no way to distinguish "backfilled by this
-- migration" from "set later via launchScene" to selectively undo. This
-- matches this repo's existing convention of non-reversible data-only
-- down migrations (see e.g. the widen_scene_grid_type_gridless down.sql
-- comment for the same reasoning applied to a constraint change).
SELECT 1;
