-- Reverses 2026-09-15-180000-0000_npc_visible_to_players. Nothing else read
-- the column, so every NPC is listed for every member again.
ALTER TABLE world_actors DROP COLUMN IF EXISTS visible_to_players;
