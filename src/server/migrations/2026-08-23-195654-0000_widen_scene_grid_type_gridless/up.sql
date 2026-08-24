-- Spec 018 (Genie house system, US2): widen scenes.grid_type to permit
-- 'gridless' alongside the existing 'square'/'hex' values.
--
-- The engine already models this topology end-to-end (see
-- `GridType::Gridless` in src/engine/src/resources/scene_data.rs and the
-- corresponding match arm in src/engine/src/plugins/grid.rs), but the
-- original scenes table migration
-- (2026-05-05-010000-0001_create_scenes_table/up.sql) only ever permitted
-- 'square'/'hex' at the database layer. This is an additive migration on
-- top of that one, not an edit to it.
ALTER TABLE scenes DROP CONSTRAINT scenes_grid_type_check;

ALTER TABLE scenes ADD CONSTRAINT scenes_grid_type_check
  CHECK (grid_type IN ('square', 'hex', 'gridless'));
