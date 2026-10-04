-- Reverses 2026-09-14-210100-0000_retire_token_health.
--
-- The columns come back with exactly the values they held, from the archive
-- the way up wrote. A copy's `system_data` is left as it is: it may have been
-- damaged since, and it is the truth about that copy until the previous
-- migration's own `down.sql` drops the column.
ALTER TABLE tokens
    ADD COLUMN health INTEGER NULL,
    ADD COLUMN max_health INTEGER NULL;

UPDATE tokens t
   SET health = r.health,
       max_health = r.max_health
  FROM retired_token_health r
 WHERE r.token_id = t.token_id;

DROP TABLE retired_token_health;
