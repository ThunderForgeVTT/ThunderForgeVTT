-- Owner decision 2026-09-15: whether players may see an NPC is the Game
-- Master's choice, per NPC.
--
-- Before this, every member of a world could list every actor in it, so a
-- player read the names of NPCs the Game Master had not introduced yet in the
-- character list, although token-name hiding already kept those names off the
-- board and the combat tracker.
--
-- `false` is the default and the backfill: every NPC that exists today is
-- hidden until its Game Master shows it, and so is every NPC written from now
-- on by a path that does not decide. The column means nothing for a player
-- character, which every member of the world still sees. The rule that reads
-- it is `auth::npc_visibility`, and nothing else reads it.
ALTER TABLE world_actors
    ADD COLUMN visible_to_players BOOLEAN NOT NULL DEFAULT false;
