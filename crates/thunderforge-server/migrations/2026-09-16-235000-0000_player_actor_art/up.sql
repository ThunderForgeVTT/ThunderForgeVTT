-- Spec 044 phase (c), FR-030a and FR-030b: the player who holds a character
-- may change its look, and a Game Master has two ways to withdraw that.
--
-- `allow_player_actor_art` defaults ON, unlike its neighbours
-- (`allow_player_created_actors`, `auto_apply_npc_damage`): the grant is the
-- feature and the setting is how a table withdraws it. The DEFAULT backfills
-- every existing world with true.
ALTER TABLE worlds
    ADD COLUMN allow_player_actor_art BOOLEAN NOT NULL DEFAULT true;

-- One character's look, locked by the Game Master whatever the world says.
-- Not carried by a collection copy: a lock is one table's Game Master telling
-- one table's player something.
ALTER TABLE world_actors
    ADD COLUMN art_locked BOOLEAN NOT NULL DEFAULT false;
