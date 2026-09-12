-- Spec 045 US7 (owner decision 3): a player's map remembers where they have
-- been, and a Game Master can reset it.
--
-- What a player has explored is NOT here. It lives in that player's own
-- browser, and clearing their storage loses it — which is theirs to lose.
-- The server holds only two things: whether a scene remembers at all, and a
-- number that tells a browser its memory is stale.

-- FR-070: off for every existing scene and every new one, until a Game Master
-- turns it on. A scene that has never heard of exploration must not suddenly
-- start hiding its map from the table.
ALTER TABLE scenes
    ADD COLUMN exploration_enabled BOOLEAN NOT NULL DEFAULT FALSE;

-- Bumped by a reset for everyone. A browser holding an older epoch drops what
-- it kept (FR-078).
--
-- An epoch rather than a broadcast, because a broadcast only reaches whoever
-- was listening: a player who was offline when the Game Master reset the fog
-- would come back with their old map intact, and never know.
ALTER TABLE scenes
    ADD COLUMN exploration_epoch INTEGER NOT NULL DEFAULT 0;

-- A reset aimed at one player.
--
-- Separate from the scene's own epoch so that resetting one player does not
-- invalidate everybody's map. A client takes the greater of the two, so a
-- later reset for everyone still reaches a player who was reset individually.
CREATE TABLE scene_exploration_resets (
    scene_id UUID NOT NULL REFERENCES scenes(scene_id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    epoch INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (scene_id, user_id)
);

-- The read is always "this scene, this player", which the primary key already
-- serves. No further index: the table is one row per player per scene that has
-- ever been reset individually, which is small.
