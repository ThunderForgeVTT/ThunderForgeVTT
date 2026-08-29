-- What a token represents, so a scene can be read at a glance.
--
-- Every token without a portrait rendered in the same blue: a player, an
-- ogre, the cart they are escorting and a barrel were visually identical.
-- The engine's `Token` component has carried a `token_type` field all along
-- ("character, npc, object, etc."); nothing ever stored it.
ALTER TABLE tokens
ADD COLUMN token_type VARCHAR NOT NULL DEFAULT 'character';

-- Backfill from the actor a token is bound to, rather than declaring every
-- existing token a player character. `world_actors.actor_type` already
-- records this distinction, so the information exists and the alternative is
-- throwing it away and asking Game Masters to re-enter it by hand.
--
-- Tokens bound to nothing, and tokens whose actor is a player character,
-- keep the column default.
UPDATE tokens
SET token_type = 'npc'
FROM world_actors
WHERE tokens.actor_id = world_actors.id
  AND world_actors.actor_type = 'npc';
