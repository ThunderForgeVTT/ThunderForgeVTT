-- Reverse: restore the original 'square'/'hex'-only constraint.
-- Any 'gridless' rows would violate this constraint; this migration does
-- not attempt to reassign them, matching the down-migration conventions
-- used elsewhere in this repo (destructive rollbacks are expected to be
-- run only against data created after the corresponding up.sql).
ALTER TABLE scenes DROP CONSTRAINT scenes_grid_type_check;

ALTER TABLE scenes ADD CONSTRAINT scenes_grid_type_check
  CHECK (grid_type IN ('square', 'hex'));
