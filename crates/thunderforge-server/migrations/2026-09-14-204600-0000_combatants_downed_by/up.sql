-- Spec 046 (research R9, contract C8): why a combatant is out of the fight.
--
-- `active = false` has meant "the Game Master pressed Down" since the tracker
-- was built. A creature taken to zero hit points is now out too, and the two
-- must not be confused: healing brings back a creature that dropped, and
-- never one the Game Master took out by hand.
--
-- NULL is "not out", or out for a reason recorded before this column existed,
-- which healing leaves alone for the same reason it leaves a manual Down.
ALTER TABLE world_combatants
    ADD COLUMN downed_by TEXT NULL
        CHECK (downed_by IN ('hit_points', 'game_master'));
