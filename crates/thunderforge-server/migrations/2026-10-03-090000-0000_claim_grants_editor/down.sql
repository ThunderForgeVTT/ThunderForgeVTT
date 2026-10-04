-- What a claim granted goes with the column that says a claim granted it.
-- Left behind, those rows would read as hand grants and outlive their claims.
DELETE FROM world_actor_permissions WHERE granted_by_claim;
ALTER TABLE world_actor_permissions DROP COLUMN IF EXISTS granted_by_claim;
