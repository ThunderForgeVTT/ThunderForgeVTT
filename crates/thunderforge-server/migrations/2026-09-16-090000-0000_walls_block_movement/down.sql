-- Reverses 2026-09-16-090000-0000_walls_block_movement: the column default
-- goes back to false and the walls the backfill changed stop blocking
-- movement again. A recorded wall deleted since is simply gone.
ALTER TABLE walls ALTER COLUMN blocks_movement SET DEFAULT false;

UPDATE walls
SET blocks_movement = false
WHERE wall_id IN (SELECT wall_id FROM walls_block_movement_backfill);

DROP TABLE walls_block_movement_backfill;
