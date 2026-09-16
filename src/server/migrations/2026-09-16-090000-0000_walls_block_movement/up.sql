-- Owner decision 2026-09-15: a drawn wall blocks movement and vision by
-- default. The wall tool (drag, chain, room), `createWall`'s default and the
-- UVTT importer all wrote `blocks_movement = false`, so every wall they made
-- was decorative to a token once spec 045 had the server enforce the flag.
--
-- Existing walls are migrated rather than left alone. A wall that blocks
-- vision but not movement was never a deliberate choice: until now it was the
-- only thing those paths could produce, and the one way to ask for it on
-- purpose (clearing "Blocks movement") leaves a row indistinguishable from
-- the defect. Counted read-only on the dev database before writing this:
-- 11,249 walls, of which 11,121 plain walls and 66 closed doors block vision
-- but not movement. The 21 walls that do not block vision all block movement,
-- so no deliberate shape anyone made resembles the defect. Leaving them would
-- keep old rooms walkable while new rooms are not.
--
-- Doors are included: a door drawn as a door already blocked movement, so a
-- door that does not came from cycling a defective wall with `O` or from the
-- importer — the same defect. `Wall::blocking` still lets any *open* door
-- through, so only closed ones change behaviour. Walls that do not block
-- vision are not touched.
--
-- The rows changed are recorded so `down.sql` reverses exactly them.
CREATE TABLE walls_block_movement_backfill (
    wall_id UUID PRIMARY KEY
);

WITH changed AS (
    UPDATE walls
    SET blocks_movement = true
    WHERE blocks_vision AND NOT blocks_movement
    RETURNING wall_id
)
INSERT INTO walls_block_movement_backfill (wall_id)
SELECT wall_id FROM changed;

-- A row written by a path that does not decide is a wall too.
ALTER TABLE walls ALTER COLUMN blocks_movement SET DEFAULT true;
