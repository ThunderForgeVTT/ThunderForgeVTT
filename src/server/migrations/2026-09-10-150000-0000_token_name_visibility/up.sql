-- Playtest 2026-09-10 P7: a token's name, drawn above it, can be hidden from
-- players by a Game Master, who always sees every name.
--
-- Shown by default: a name is what a player expects to read, and hiding one
-- is the exception a Game Master chooses — a monster the party has not
-- identified, a disguise. The server withholds a hidden name from every
-- client but a Game Master's, on the token and in the combat tracker.
ALTER TABLE tokens
    ADD COLUMN name_visible_to_players BOOLEAN NOT NULL DEFAULT true;
